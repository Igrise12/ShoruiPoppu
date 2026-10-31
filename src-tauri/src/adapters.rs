use pulldown_cmark::{Event, Options, Parser, Tag};
use serde::Serialize;
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
    sync::Arc,
};

const MAX_FILE_SIZE: u64 = 100 * 1024 * 1024;

#[derive(Serialize)]
pub(crate) struct Token {
    kind: String,
    text: String,
}

#[derive(Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub(crate) enum ViewerState {
    Empty,
    Pdf {
        name: String,
        bytes: Vec<u8>,
    },
    Markdown {
        name: String,
        tokens: Vec<Token>,
    },
    Text {
        name: String,
        text: String,
    },
    Grid {
        name: String,
        sheets: Vec<String>,
        sheet: String,
        rows: Vec<Vec<String>>,
        row_offset: u32,
        column_offset: u32,
    },
    Workbook {
        name: String,
        sheets: Vec<String>,
        sheet: String,
        row_offset: u32,
        column_offset: u32,
        row_count: u32,
        column_count: u32,
        complete: bool,
    },
    Docx {
        name: String,
        document: crate::office::WordDocument,
    },
    Pptx {
        name: String,
        index: usize,
        count: usize,
        slide: crate::office::Slide,
    },
    Unsupported {
        name: String,
    },
    Error {
        name: String,
        message: String,
    },
}

fn token(kind: &str, text: impl Into<String>) -> Token {
    Token {
        kind: kind.into(),
        text: text.into(),
    }
}

fn markdown(input: &str) -> Vec<Token> {
    let parser = Parser::new_ext(
        input,
        Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH,
    );
    let mut tokens = Vec::new();
    for event in parser {
        let next = match event {
            Event::Start(tag) => {
                let element = match tag {
                    Tag::Paragraph => "p",
                    Tag::Heading { level, .. } => match level as u8 {
                        1 => "h1",
                        2 => "h2",
                        3 => "h3",
                        4 => "h4",
                        5 => "h5",
                        _ => "h6",
                    },
                    Tag::BlockQuote(_) => "blockquote",
                    Tag::CodeBlock(_) => "pre",
                    Tag::List(Some(_)) => "ol",
                    Tag::List(None) => "ul",
                    Tag::Item => "li",
                    Tag::Emphasis => "em",
                    Tag::Strong => "strong",
                    Tag::Strikethrough => "del",
                    Tag::Table(_) => "table",
                    Tag::TableHead => "thead",
                    Tag::TableRow => "tr",
                    Tag::TableCell => "td",
                    Tag::Link { .. } => "span",
                    Tag::Image { .. } => "span",
                    _ => "span",
                };
                token("start", element)
            }
            Event::End(_) => token("end", ""),
            Event::Text(text) | Event::Code(text) => token("text", text.to_string()),
            Event::SoftBreak | Event::HardBreak => token("break", ""),
            Event::Rule => token("rule", ""),
            Event::Html(_) | Event::InlineHtml(_) => continue,
            _ => continue,
        };
        tokens.push(next);
    }
    tokens
}

fn name(path: &Path) -> String {
    path.file_name()
        .unwrap_or(path.as_os_str())
        .to_string_lossy()
        .into_owned()
}

fn validate(path: &Path) -> Result<(), String> {
    let metadata = fs::metadata(path).map_err(|e| {
        match e.kind() {
            std::io::ErrorKind::NotFound => "The file could not be found.",
            std::io::ErrorKind::PermissionDenied => "You do not have permission to read this file.",
            _ => "The file could not be opened.",
        }
        .to_string()
    })?;
    if !metadata.is_file() {
        return Err("Please choose a regular file.".into());
    }
    if metadata.len() > MAX_FILE_SIZE {
        return Err("This file exceeds the 100 MB viewing limit.".into());
    }
    fs::File::open(path).map_err(|e| {
        match e.kind() {
            std::io::ErrorKind::PermissionDenied => "You do not have permission to read this file.",
            _ => "The file could not be opened.",
        }
        .to_string()
    })?;
    Ok(())
}

fn read(path: &Path) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    fs::File::open(path)
        .map_err(|_| "The file could not be read.".to_string())?
        .take(MAX_FILE_SIZE + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "The file could not be read.".to_string())?;
    if bytes.len() as u64 > MAX_FILE_SIZE {
        return Err("This file exceeds the 100 MB viewing limit.".into());
    }
    Ok(bytes)
}

pub(crate) fn inspect_file(path: &Path) -> ViewerState {
    let name = name(path);
    if let Err(message) = validate(path) {
        return ViewerState::Error { name, message };
    }
    let extension = path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let result = match extension.as_str() {
        "pdf" => read(path).map(|bytes| ViewerState::Pdf {
            name: name.clone(),
            bytes,
        }),
        "md" | "txt" | "csv" => match read(path).and_then(|bytes| {
            String::from_utf8(bytes).map_err(|_| "This text file is not valid UTF-8.".to_string())
        }) {
            Ok(text) if extension == "md" => Ok(ViewerState::Markdown {
                name: name.clone(),
                tokens: markdown(&text),
            }),
            Ok(text) if extension == "txt" => Ok(ViewerState::Text {
                name: name.clone(),
                text,
            }),
            Ok(text) => {
                let mut reader = csv::ReaderBuilder::new()
                    .has_headers(false)
                    .flexible(true)
                    .from_reader(text.as_bytes());
                let rows: Result<Vec<Vec<String>>, _> = reader
                    .records()
                    .map(|r| r.map(|row| row.iter().map(str::to_owned).collect()))
                    .collect();
                rows.map(|rows| ViewerState::Grid {
                    name: name.clone(),
                    sheets: vec![],
                    sheet: String::new(),
                    rows,
                    row_offset: 0,
                    column_offset: 0,
                })
                .map_err(|_| "The CSV file could not be read.".into())
            }
            Err(message) => Err(message),
        },
        "xls" | "xlsx" | "pptx" => Err("This format requires a document session.".into()),
        "docx" => crate::office::docx(path).map(|document| ViewerState::Docx {
            name: name.clone(),
            document,
        }),
        _ => return ViewerState::Unsupported { name },
    };
    result.unwrap_or_else(|message| ViewerState::Error { name, message })
}

#[derive(Clone, Serialize)]
pub(crate) struct Capabilities {
    pub search: bool,
    pub zoom: bool,
    pub worksheets: bool,
    pub slides: bool,
}
#[derive(Serialize)]
pub(crate) struct Response {
    pub content: Arc<ViewerState>,
    pub capabilities: Capabilities,
}

enum Adapter {
    None,
    Workbook(crate::workbook::Workbook),
    Presentation(crate::office::Presentation),
}
pub(crate) struct Session {
    path: Option<PathBuf>,
    current: Option<Arc<ViewerState>>,
    adapter: Adapter,
}
impl Session {
    pub fn new(path: Option<PathBuf>) -> Self {
        Self {
            path,
            current: None,
            adapter: Adapter::None,
        }
    }
    fn response(&self) -> Response {
        let content = self.current.as_ref().expect("session is opened").clone();
        let capabilities = Capabilities {
            search: matches!(&*content, ViewerState::Pdf { .. }),
            zoom: matches!(&*content, ViewerState::Pdf { .. }),
            worksheets: matches!(&*content, ViewerState::Workbook { .. }),
            slides: matches!(&*content, ViewerState::Pptx { .. }),
        };
        Response {
            content,
            capabilities,
        }
    }
    fn workbook_view(
        name: String,
        workbook: &crate::workbook::Workbook,
    ) -> Result<ViewerState, String> {
        let view = workbook.snapshot(0, 0, 0, 0)?;
        Ok(ViewerState::Workbook {
            name,
            sheets: workbook.sheets.clone(),
            sheet: workbook.sheet.clone(),
            row_offset: view.row_offset,
            column_offset: view.column_offset,
            row_count: view.row_count,
            column_count: view.column_count,
            complete: view.complete,
        })
    }
    pub fn open(&mut self) -> Response {
        if self.current.is_none() {
            let state = match self.path.as_ref() {
                None => ViewerState::Empty,
                Some(path) => {
                    let file_name = name(path);
                    let extension = path
                        .extension()
                        .and_then(|s| s.to_str())
                        .unwrap_or("")
                        .to_ascii_lowercase();
                    let result = validate(path).and_then(|_| match extension.as_str() {
                        "xls" | "xlsx" => {
                            let workbook = crate::workbook::Workbook::open(path)?;
                            let view = Self::workbook_view(file_name.clone(), &workbook)?;
                            self.adapter = Adapter::Workbook(workbook);
                            Ok(view)
                        }
                        "pptx" => {
                            let mut presentation = crate::office::Presentation::open(path)?;
                            let (count, slide) = presentation.slide(0)?;
                            self.adapter = Adapter::Presentation(presentation);
                            Ok(ViewerState::Pptx {
                                name: file_name.clone(),
                                index: 0,
                                count,
                                slide,
                            })
                        }
                        _ => Ok(inspect_file(path)),
                    });
                    result.unwrap_or_else(|message| ViewerState::Error {
                        name: file_name,
                        message,
                    })
                }
            };
            self.current = Some(Arc::new(state));
        }
        self.response()
    }
    pub fn worksheet(&mut self, sheet: &str) -> Result<Response, String> {
        let Adapter::Workbook(workbook) = &mut self.adapter else {
            return Err("This document is not a workbook.".into());
        };
        workbook.select(sheet)?;
        self.current = Some(Arc::new(Self::workbook_view(
            name(self.path.as_ref().expect("open workbook has a path")),
            workbook,
        )?));
        Ok(self.response())
    }
    pub fn viewport(
        &self,
        sheet: &str,
        row: u32,
        rows: u32,
        column: u32,
        columns: u32,
    ) -> Result<crate::workbook::GridSnapshot, String> {
        let Adapter::Workbook(workbook) = &self.adapter else {
            return Err("This document is not a workbook.".into());
        };
        if workbook.sheet != sheet {
            return Err("The active worksheet has changed.".into());
        }
        workbook.snapshot(row, rows, column, columns)
    }
    pub fn slide(&mut self, index: usize) -> Result<Response, String> {
        let Adapter::Presentation(presentation) = &mut self.adapter else {
            return Err("This document is not a presentation.".into());
        };
        let (count, slide) = presentation.slide(index)?;
        self.current = Some(Arc::new(ViewerState::Pptx {
            name: name(self.path.as_ref().expect("open presentation has a path")),
            index,
            count,
            slide,
        }));
        Ok(self.response())
    }
    pub fn slide_preview(&mut self, index: usize) -> Result<crate::office::Slide, String> {
        let Adapter::Presentation(presentation) = &mut self.adapter else {
            return Err("This document is not a presentation.".into());
        };
        presentation.slide(index).map(|(_, slide)| slide)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn session_keeps_open_content_and_reports_capabilities() {
        let path =
            std::env::temp_dir().join(format!("shoruipoppu-session-{}.txt", std::process::id()));
        std::fs::write(&path, "initial content").unwrap();
        let mut session = Session::new(Some(path.clone()));
        let first = session.open();
        std::fs::remove_file(path).unwrap();
        let second = session.open();
        assert!(Arc::ptr_eq(&first.content, &second.content));
        assert!(
            matches!(&*second.content, ViewerState::Text { text, .. } if text == "initial content")
        );
        assert!(!second.capabilities.search && !second.capabilities.worksheets);
        assert!(session.worksheet("other").is_err());
        assert!(session.slide(0).is_err());
    }
    #[test]
    fn slide_previews_do_not_change_the_selected_slide() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/example.pptx");
        let mut session = Session::new(Some(path));
        let first = session.open();
        assert!(matches!(
            &*first.content,
            ViewerState::Pptx { index: 0, .. }
        ));
        assert!(session.slide_preview(1).is_ok());
        assert!(session.slide_preview(2).is_err());
        assert!(Arc::ptr_eq(&first.content, &session.open().content));
    }

    #[test]
    fn markdown_drops_html_and_preserves_text_and_tables() {
        let tokens =
            markdown("# Title\n\n<script>bad()</script>\n\n| a | b |\n| - | - |\n| c | d |");
        assert!(tokens.iter().any(|t| t.text == "h1"));
        assert!(tokens.iter().any(|t| t.text == "table"));
        assert!(!tokens
            .iter()
            .any(|t| t.text.contains("script") || t.text.contains("bad()")));
    }
    #[test]
    fn reads_text_and_csv_and_rejects_invalid_utf8() {
        let dir = std::env::temp_dir();
        let prefix = format!("shoruipoppu-adapter-{}", std::process::id());
        let text = dir.join(format!("{prefix}.txt"));
        let csv = dir.join(format!("{prefix}.csv"));
        std::fs::write(&text, "hello").unwrap();
        std::fs::write(&csv, "name,value\nquoted,\"a,b\"\n").unwrap();
        assert!(matches!(inspect_file(&text), ViewerState::Text { text, .. } if text == "hello"));
        assert!(
            matches!(inspect_file(&csv), ViewerState::Grid { rows, .. } if rows[1][1] == "a,b")
        );
        std::fs::write(&text, [0xff]).unwrap();
        assert!(matches!(inspect_file(&text), ViewerState::Error { .. }));
        std::fs::remove_file(text).unwrap();
        std::fs::remove_file(csv).unwrap();
    }

    #[test]
    fn reads_workbook_cells_with_original_coordinates() {
        use std::io::Write;
        let path =
            std::env::temp_dir().join(format!("shoruipoppu-workbook-{}.xlsx", std::process::id()));
        let mut zip = zip::ZipWriter::new(fs::File::create(&path).unwrap());
        for (name, xml) in [
            (
                "_rels/.rels",
                r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/></Relationships>"#,
            ),
            (
                "xl/workbook.xml",
                r#"<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><sheets><sheet name="Sheet One" sheetId="1" r:id="rId1"/><sheet name="Sheet Two" sheetId="2" r:id="rId2"/><sheet name="Broken" sheetId="3" r:id="rId3"/></sheets></workbook>"#,
            ),
            (
                "xl/_rels/workbook.xml.rels",
                r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet2.xml"/><Relationship Id="rId3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet3.xml"/></Relationships>"#,
            ),
            (
                "xl/worksheets/sheet1.xml",
                r#"<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData><row r="3"><c r="C3" t="inlineStr"><is><t>Hello</t></is></c></row><row r="1000000"><c r="XFD1000000" t="inlineStr"><is><t>Far away</t></is></c></row></sheetData></worksheet>"#,
            ),
            (
                "xl/worksheets/sheet2.xml",
                r#"<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData><row r="1"><c r="A1" t="inlineStr"><is><t>Second</t></is></c></row></sheetData></worksheet>"#,
            ),
            (
                "xl/worksheets/sheet3.xml",
                r#"<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData><row r="1"><c r="A1"><v>1</v></c></row><row r="2"><c r="XFE2"><v>2</v></c></row></sheetData></worksheet>"#,
            ),
        ] {
            zip.start_file(name, zip::write::SimpleFileOptions::default())
                .unwrap();
            zip.write_all(xml.as_bytes()).unwrap();
        }
        zip.finish().unwrap();
        let mut session = Session::new(Some(path.clone()));
        let result = session.open();
        assert!(matches!(
            &*result.content,
            ViewerState::Workbook {
                row_offset: 2,
                column_offset: 2,
                ..
            }
        ));
        let cells = session.viewport("Sheet One", 0, 1, 0, 1).unwrap();
        assert_eq!(cells.cells[0].value, "Hello");
        while !session.viewport("Sheet One", 0, 0, 0, 0).unwrap().complete {
            std::thread::yield_now();
        }
        let far = session
            .viewport("Sheet One", 999_997, 1, 16_381, 1)
            .unwrap();
        assert_eq!(far.cells[0].value, "Far away");
        assert!(session.viewport("Sheet One", 0, 129, 0, 1).is_err());
        std::fs::remove_file(path).unwrap();
        assert!(session.worksheet("missing").is_err());
        assert_eq!(
            session.viewport("Sheet One", 0, 1, 0, 1).unwrap().cells[0].value,
            "Hello"
        );
        session.worksheet("Sheet Two").unwrap();
        assert!(session.viewport("Sheet One", 0, 1, 0, 1).is_err());
        assert_eq!(
            session.viewport("Sheet Two", 0, 1, 0, 1).unwrap().cells[0].value,
            "Second"
        );
        session.worksheet("Broken").unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            let view = session.viewport("Broken", 0, 1, 0, 1).unwrap();
            if view.complete {
                assert!(view.error.is_some());
                assert_eq!(view.cells[0].value, "1");
                break;
            }
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
    }

    #[test]
    fn large_xlsx_returns_only_requested_cells() {
        use std::io::Write;
        let path =
            std::env::temp_dir().join(format!("shoruipoppu-large-{}.xlsx", std::process::id()));
        let mut zip = zip::ZipWriter::new(fs::File::create(&path).unwrap());
        let options = zip::write::SimpleFileOptions::default();
        for (name, xml) in [
            (
                "_rels/.rels",
                r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/></Relationships>"#,
            ),
            (
                "xl/workbook.xml",
                r#"<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><sheets><sheet name="Large" sheetId="1" r:id="rId1"/><sheet name="Other" sheetId="2" r:id="rId2"/></sheets></workbook>"#,
            ),
            (
                "xl/_rels/workbook.xml.rels",
                r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet2.xml"/></Relationships>"#,
            ),
        ] {
            zip.start_file(name, options).unwrap();
            zip.write_all(xml.as_bytes()).unwrap();
        }
        zip.start_file("xl/worksheets/sheet1.xml", options).unwrap();
        zip.write_all(br#"<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData>"#).unwrap();
        for row in 1..=50_000 {
            write!(
                zip,
                "<row r=\"{row}\"><c r=\"A{row}\"><v>{row}</v></c></row>"
            )
            .unwrap();
        }
        zip.write_all(b"</sheetData></worksheet>").unwrap();
        zip.start_file("xl/worksheets/sheet2.xml", options).unwrap();
        zip.write_all(br#"<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData><row r="1"><c r="A1"><v>42</v></c></row></sheetData></worksheet>"#).unwrap();
        zip.finish().unwrap();
        let mut session = Session::new(Some(path.clone()));
        let response = session.open();
        assert!(matches!(
            &*response.content,
            ViewerState::Workbook {
                row_count: 1..=50_000,
                ..
            }
        ));
        assert_eq!(
            session.viewport("Large", 0, 1, 0, 1).unwrap().cells[0].value,
            "1"
        );
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
        loop {
            let status = session.viewport("Large", 0, 0, 0, 0).unwrap();
            if status.complete {
                assert!(status.error.is_none());
                assert_eq!(status.row_count, 50_000);
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "worksheet scan did not finish"
            );
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        let cells = session.viewport("Large", 30_000, 20, 0, 1).unwrap();
        assert_eq!(cells.cells.len(), 20);
        assert_eq!(cells.cells[0].value, "30001");
        let mut switched = Session::new(Some(path.clone()));
        switched.open();
        switched.worksheet("Other").unwrap();
        assert_eq!(
            switched.viewport("Other", 0, 1, 0, 1).unwrap().cells[0].value,
            "42"
        );
        assert!(switched.viewport("Large", 0, 1, 0, 1).is_err());
        drop(switched);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn damaged_office_files_report_errors_without_changing_sources() {
        let dir = std::env::temp_dir().join(format!("shoruipoppu-damaged-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        for extension in ["xlsx", "docx", "pptx"] {
            let path = dir.join(format!("invalid.{extension}"));
            std::fs::write(&path, b"not a document").unwrap();
            let mut session = Session::new(Some(path.clone()));
            assert!(matches!(
                &*session.open().content,
                ViewerState::Error { .. }
            ));
            assert_eq!(std::fs::read(&path).unwrap(), b"not a document");
        }
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 3);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn missing_and_unknown_files_report_errors() {
        assert!(matches!(
            inspect_file(Path::new("/path/does/not/exist.pdf")),
            ViewerState::Error { .. }
        ));
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        assert!(matches!(
            inspect_file(&root.join("package.json")),
            ViewerState::Unsupported { .. }
        ));
        assert!(matches!(
            inspect_file(&root.join("to-do.md")),
            ViewerState::Markdown { .. }
        ));
    }
}
