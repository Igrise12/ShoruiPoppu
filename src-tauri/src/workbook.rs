use calamine::{Reader, Xls, Xlsx};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs::File,
    io::{BufReader, Read, Seek, SeekFrom},
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc, Mutex,
    },
    thread::{self, JoinHandle},
};

const MAX_CELLS: usize = 1_000_000;
const MAX_TEXT: usize = 64 * 1024 * 1024;
const MAX_UNCOMPRESSED: u64 = 128 * 1024 * 1024;
const MAX_PART: u64 = 64 * 1024 * 1024;
const MAX_ROWS: u32 = 1_048_576;
const MAX_COLUMNS: u32 = 16_384;

#[derive(Serialize)]
pub(crate) struct Cell {
    pub row: u32,
    pub column: u32,
    pub value: String,
}

#[derive(Default, Serialize, Deserialize)]
pub(crate) struct SparseSheet {
    rows: BTreeMap<u32, BTreeMap<u32, String>>,
    count: usize,
    text_bytes: usize,
    min_row: u32,
    max_row: u32,
    min_column: u32,
    max_column: u32,
}

impl SparseSheet {
    fn insert(&mut self, row: u32, column: u32, value: String) -> Result<(), String> {
        if value.is_empty() {
            return Ok(());
        }
        if row >= MAX_ROWS || column >= MAX_COLUMNS {
            return Err("The worksheet contains an invalid cell position.".into());
        }
        let old = self.rows.get(&row).and_then(|r| r.get(&column));
        let new_count = self.count + usize::from(old.is_none());
        let new_bytes = self.text_bytes - old.map_or(0, String::len) + value.len();
        if new_count > MAX_CELLS || new_bytes > MAX_TEXT {
            return Err(
                "The worksheet exceeds the viewing limit (1 million cells or 64 MB of cell text)."
                    .into(),
            );
        }
        if self.count == 0 {
            self.min_row = row;
            self.max_row = row;
            self.min_column = column;
            self.max_column = column;
        } else {
            self.min_row = self.min_row.min(row);
            self.max_row = self.max_row.max(row);
            self.min_column = self.min_column.min(column);
            self.max_column = self.max_column.max(column);
        }
        self.rows.entry(row).or_default().insert(column, value);
        self.count = new_count;
        self.text_bytes = new_bytes;
        Ok(())
    }
    fn bounds(&self) -> (u32, u32, u32, u32) {
        if self.count == 0 {
            (0, 0, 0, 0)
        } else {
            (
                self.min_row,
                self.min_column,
                self.max_row - self.min_row + 1,
                self.max_column - self.min_column + 1,
            )
        }
    }
    pub(crate) fn viewport(
        &self,
        start_row: u32,
        row_count: u32,
        start_column: u32,
        column_count: u32,
    ) -> Result<Vec<Cell>, String> {
        if row_count > 128 || column_count > 64 {
            return Err("The requested grid area is too large.".into());
        }
        let (row_offset, column_offset, rows, columns) = self.bounds();
        if start_row >= rows || start_column >= columns || row_count == 0 || column_count == 0 {
            return Ok(Vec::new());
        }
        let first_row = row_offset + start_row;
        let last_row = first_row + row_count.min(rows - start_row);
        let first_column = column_offset + start_column;
        let last_column = first_column + column_count.min(columns - start_column);
        let mut cells = Vec::new();
        for (&row, columns) in self.rows.range(first_row..last_row) {
            for (&column, value) in columns.range(first_column..last_column) {
                cells.push(Cell {
                    row: row - row_offset,
                    column: column - column_offset,
                    value: value.clone(),
                });
            }
        }
        Ok(cells)
    }
}

#[derive(Serialize, Deserialize)]
pub(crate) struct XlsResult {
    pub(crate) sheets: Vec<String>,
    pub(crate) data: SparseSheet,
}

// Calamine loads every XLS sheet eagerly. Isolate that parser so a sparse or
// damaged legacy workbook cannot exhaust the viewer process.
#[cfg(target_os = "linux")]
fn read_xls(file: &File, sheet: Option<&str>) -> Result<XlsResult, String> {
    let executable = std::env::current_exe().map_err(|_| "Could not start the XLS reader.")?;
    read_xls_with_executable(file, sheet, &executable)
}
#[cfg(target_os = "linux")]
pub(crate) fn read_xls_with_executable(
    file: &File,
    sheet: Option<&str>,
    executable: &Path,
) -> Result<XlsResult, String> {
    use std::{
        os::fd::AsRawFd,
        os::unix::process::CommandExt,
        process::{Command, Stdio},
    };
    let fd = file.as_raw_fd();
    let mut command = Command::new(executable);
    command
        .arg("--internal-xls-reader")
        .arg(format!("/proc/self/fd/{fd}"));
    if let Some(sheet) = sheet {
        command.arg(sheet);
    }
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    unsafe {
        command.pre_exec(move || {
            let limit = libc::rlimit {
                rlim_cur: 512 * 1024 * 1024,
                rlim_max: 512 * 1024 * 1024,
            };
            let cpu = libc::rlimit {
                rlim_cur: 30,
                rlim_max: 30,
            };
            let no_core = libc::rlimit {
                rlim_cur: 0,
                rlim_max: 0,
            };
            if libc::setrlimit(libc::RLIMIT_AS, &limit) != 0
                || libc::setrlimit(libc::RLIMIT_CPU, &cpu) != 0
                || libc::setrlimit(libc::RLIMIT_CORE, &no_core) != 0
                || libc::fcntl(fd, libc::F_SETFD, 0) == -1
            {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let mut child = command
        .spawn()
        .map_err(|_| "Could not start the XLS reader.")?;
    let mut output = Vec::new();
    let read = child
        .stdout
        .take()
        .ok_or("Could not start the XLS reader.")?
        .take(128 * 1024 * 1024 + 1)
        .read_to_end(&mut output);
    if read.is_err() || output.len() > 128 * 1024 * 1024 {
        let _ = child.kill();
        let _ = child.wait();
        return Err("The XLS worksheet exceeds the viewing limit.".into());
    }
    let status = child.wait().map_err(|_| "The XLS reader did not finish.")?;
    if !status.success() {
        return Err("The XLS workbook is damaged or exceeds the memory limit.".into());
    }
    serde_json::from_slice::<Result<XlsResult, String>>(&output)
        .map_err(|_| "The XLS reader returned invalid data.".to_string())?
}
#[cfg(not(target_os = "linux"))]
fn read_xls(_file: &File, _sheet: Option<&str>) -> Result<XlsResult, String> {
    Err("XLS viewing is currently available on Linux only.".into())
}

pub(crate) fn run_xls_reader(path: &str, selected: Option<&str>) -> Result<(), String> {
    let result = (|| {
        let file = File::open(path).map_err(|_| "The workbook could not be opened.")?;
        let mut workbook = Xls::new(BufReader::new(file))
            .map_err(|_| "The workbook is damaged, encrypted, or cannot be read.")?;
        let sheets = workbook.sheet_names();
        let sheet = selected
            .map(str::to_owned)
            .or_else(|| sheets.first().cloned())
            .ok_or("The workbook contains no worksheets.")?;
        if !sheets.contains(&sheet) {
            return Err("The worksheet could not be found.".to_string());
        }
        let range = workbook
            .worksheet_range(&sheet)
            .map_err(|_| "The worksheet could not be read.")?;
        let (row, column) = range.start().unwrap_or((0, 0));
        let mut data = SparseSheet::default();
        for (r, c, value) in range.used_cells() {
            data.insert(row + r as u32, column + c as u32, value.to_string())?;
        }
        Ok(XlsResult { sheets, data })
    })();
    serde_json::to_writer(std::io::stdout().lock(), &result)
        .map_err(|_| "The XLS reader could not return the worksheet.".into())
}

enum WorkbookReader {
    Xlsx(Xlsx<BufReader<File>>),
    Xls(File),
}
#[derive(Default)]
struct ScanState {
    data: SparseSheet,
    complete: bool,
    error: Option<String>,
}
struct Scanner {
    cancel: Arc<AtomicBool>,
    thread: JoinHandle<Xlsx<BufReader<File>>>,
}
#[derive(Serialize)]
pub(crate) struct GridSnapshot {
    pub cells: Vec<Cell>,
    pub row_offset: u32,
    pub column_offset: u32,
    pub row_count: u32,
    pub column_count: u32,
    pub complete: bool,
    pub error: Option<String>,
}
pub(crate) struct Workbook {
    reader: Option<WorkbookReader>,
    scanner: Option<Scanner>,
    pub sheets: Vec<String>,
    pub sheet: String,
    data: Arc<Mutex<ScanState>>,
}
impl Drop for Workbook {
    fn drop(&mut self) {
        if let Some(scanner) = &self.scanner {
            scanner.cancel.store(true, Ordering::Relaxed);
        }
    }
}

fn check_xlsx(file: &mut File) -> Result<(), String> {
    let copy = file
        .try_clone()
        .map_err(|_| "The workbook could not be read.")?;
    let mut archive = zip::ZipArchive::new(copy)
        .map_err(|_| "The workbook is damaged, encrypted, or cannot be read.")?;
    let mut total = 0u64;
    for index in 0..archive.len() {
        let part = archive
            .by_index(index)
            .map_err(|_| "The workbook contains an unreadable part.")?;
        total = total
            .checked_add(part.size())
            .ok_or("The workbook is too large to read.")?;
        if part.size() > MAX_PART || total > MAX_UNCOMPRESSED {
            return Err("The workbook exceeds the uncompressed size limit.".into());
        }
    }
    drop(archive);
    file.seek(SeekFrom::Start(0))
        .map_err(|_| "The workbook could not be read.")?;
    Ok(())
}
impl Workbook {
    pub fn open(path: &Path) -> Result<Self, String> {
        let mut file = File::open(path).map_err(|_| "The workbook could not be opened.")?;
        let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");
        let reader = if ext.eq_ignore_ascii_case("xlsx") {
            check_xlsx(&mut file)?;
            WorkbookReader::Xlsx(
                Xlsx::new(BufReader::new(file))
                    .map_err(|_| "The workbook is damaged, encrypted, or cannot be read.")?,
            )
        } else if ext.eq_ignore_ascii_case("xls") {
            WorkbookReader::Xls(file)
        } else {
            return Err("This is not a supported workbook.".into());
        };
        let (sheets, initial_data) = match &reader {
            WorkbookReader::Xlsx(w) => (w.sheet_names(), None),
            WorkbookReader::Xls(file) => {
                let result = read_xls(file, None)?;
                (result.sheets, Some(result.data))
            }
        };
        let first = sheets
            .first()
            .cloned()
            .ok_or("The workbook contains no worksheets.")?;
        let mut workbook = Self {
            reader: Some(reader),
            scanner: None,
            sheets,
            sheet: String::new(),
            data: Arc::new(Mutex::new(ScanState::default())),
        };
        if let Some(data) = initial_data {
            workbook.sheet = first;
            workbook.data = Arc::new(Mutex::new(ScanState {
                data,
                complete: true,
                error: None,
            }));
        } else {
            workbook.select(&first)?;
        }
        Ok(workbook)
    }
    fn stop_scan(&mut self) -> Result<(), String> {
        if let Some(scanner) = self.scanner.take() {
            scanner.cancel.store(true, Ordering::Relaxed);
            let reader = scanner
                .thread
                .join()
                .map_err(|_| "The worksheet reader stopped unexpectedly.")?;
            self.reader = Some(WorkbookReader::Xlsx(reader));
        }
        Ok(())
    }
    pub fn select(&mut self, sheet: &str) -> Result<(), String> {
        if !self.sheets.iter().any(|s| s == sheet) {
            return Err("The worksheet could not be found.".into());
        }
        self.stop_scan()?;
        let reader = self
            .reader
            .take()
            .ok_or("The workbook reader is unavailable.")?;
        match reader {
            WorkbookReader::Xlsx(mut workbook) => {
                let data = Arc::new(Mutex::new(ScanState::default()));
                let target = data.clone();
                let cancel = Arc::new(AtomicBool::new(false));
                let cancelled = cancel.clone();
                let name = sheet.to_owned();
                let (ready, received) = mpsc::sync_channel(1);
                let thread = thread::spawn(move || {
                    let mut notified = false;
                    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(
                        || -> Result<(), String> {
                            let mut cells = workbook
                                .worksheet_cells_reader(&name)
                                .map_err(|_| "The worksheet could not be read.")?;
                            let mut batch = Vec::with_capacity(256);
                            while !cancelled.load(Ordering::Relaxed) {
                                let Some(cell) = cells.next_cell().map_err(|_| {
                                    "The worksheet is damaged or could not be read."
                                })?
                                else {
                                    break;
                                };
                                let (row, column) = cell.get_position();
                                let value =
                                    calamine::Data::from(cell.get_value().clone()).to_string();
                                if !value.is_empty() {
                                    batch.push((row, column, value));
                                }
                                if !batch.is_empty() && (!notified || batch.len() >= 256) {
                                    let mut state = target
                                        .lock()
                                        .map_err(|_| "Could not access the worksheet.")?;
                                    for (r, c, value) in batch.drain(..) {
                                        state.data.insert(r, c, value)?;
                                    }
                                    drop(state);
                                    if !notified {
                                        let _ = ready.send(());
                                        notified = true;
                                    }
                                }
                            }
                            let mut state = target
                                .lock()
                                .map_err(|_| "Could not access the worksheet.")?;
                            for (r, c, value) in batch {
                                state.data.insert(r, c, value)?;
                            }
                            Ok(())
                        },
                    ))
                    .unwrap_or_else(|_| Err("The worksheet reader stopped unexpectedly.".into()));
                    if let Ok(mut state) = target.lock() {
                        state.complete = true;
                        state.error = result.err();
                    }
                    if !notified {
                        let _ = ready.send(());
                    }
                    workbook
                });
                self.scanner = Some(Scanner { cancel, thread });
                received
                    .recv()
                    .map_err(|_| "The worksheet reader stopped unexpectedly.")?;
                let error = {
                    let state = data.lock().map_err(|_| "Could not access the worksheet.")?;
                    if state.data.count == 0 {
                        state.error.clone()
                    } else {
                        None
                    }
                };
                if let Some(error) = error {
                    self.stop_scan()?;
                    return Err(error);
                }
                self.data = data;
            }
            WorkbookReader::Xls(file) => {
                let result = read_xls(&file, Some(sheet));
                self.reader = Some(WorkbookReader::Xls(file));
                self.data = Arc::new(Mutex::new(ScanState {
                    data: result?.data,
                    complete: true,
                    error: None,
                }));
            }
        }
        self.sheet = sheet.to_owned();
        Ok(())
    }
    pub fn snapshot(
        &self,
        row: u32,
        rows: u32,
        column: u32,
        columns: u32,
    ) -> Result<GridSnapshot, String> {
        let state = self
            .data
            .lock()
            .map_err(|_| "Could not access the worksheet.")?;
        let (row_offset, column_offset, row_count, column_count) = state.data.bounds();
        Ok(GridSnapshot {
            cells: state.data.viewport(row, rows, column, columns)?,
            row_offset,
            column_offset,
            row_count,
            column_count,
            complete: state.complete,
            error: state.error.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sparse_sheet_keeps_distant_cells_and_limits_viewport() {
        let mut sheet = SparseSheet::default();
        sheet.insert(0, 0, "first".into()).unwrap();
        sheet.insert(1_048_575, 16_383, "last".into()).unwrap();
        assert_eq!(sheet.rows.len(), 2);
        assert_eq!(sheet.bounds(), (0, 0, 1_048_576, 16_384));
        assert_eq!(
            sheet.viewport(1_048_575, 1, 16_383, 1).unwrap()[0].value,
            "last"
        );
        assert!(sheet.viewport(0, 129, 0, 1).is_err());
        assert!(sheet.viewport(0, 1, 0, 65).is_err());
        assert!(sheet.insert(1_048_576, 0, "invalid".into()).is_err());
    }

    #[test]
    fn sparse_sheet_enforces_cell_and_text_limits() {
        let mut sheet = SparseSheet::default();
        sheet.count = MAX_CELLS;
        assert!(sheet.insert(0, 0, "one".into()).is_err());
        sheet.count = 0;
        sheet.text_bytes = MAX_TEXT;
        assert!(sheet.insert(0, 0, "one".into()).is_err());
    }
}
