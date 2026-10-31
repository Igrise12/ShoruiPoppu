#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::Instant,
};
use tauri::{Manager, State, WebviewUrl, WebviewWindow, WebviewWindowBuilder, WindowEvent};
use tauri_plugin_dialog::DialogExt;

mod adapters;
mod office;
mod workbook;
use adapters::{Response, Session};

struct ViewerWindows {
    started: Instant,
    next_id: AtomicU64,
    files: Mutex<HashMap<String, Arc<WindowSession>>>,
}

impl Default for ViewerWindows {
    fn default() -> Self {
        Self {
            started: Instant::now(),
            next_id: AtomicU64::new(0),
            files: Mutex::new(HashMap::new()),
        }
    }
}

struct WindowSession {
    document: Mutex<Session>,
    started: Instant,
    window_reported: AtomicBool,
    content_reported: AtomicBool,
}
impl WindowSession {
    fn new(path: Option<PathBuf>, started: Instant) -> Self {
        Self {
            document: Mutex::new(Session::new(path)),
            started,
            window_reported: AtomicBool::new(false),
            content_reported: AtomicBool::new(false),
        }
    }
}

fn paths_from_args(args: impl IntoIterator<Item = String>, cwd: &Path) -> Vec<PathBuf> {
    args.into_iter()
        .skip(1)
        .map(|arg| {
            let path = PathBuf::from(arg);
            if path.is_absolute() {
                path
            } else {
                cwd.join(path)
            }
        })
        .collect()
}

fn open_window(app: &tauri::AppHandle, path: Option<PathBuf>) -> tauri::Result<()> {
    let state = app.state::<ViewerWindows>();
    let id = state.next_id.fetch_add(1, Ordering::Relaxed);
    let label = format!("viewer-{id}");
    let started = if id == 0 {
        state.started
    } else {
        Instant::now()
    };
    state
        .files
        .lock()
        .unwrap()
        .insert(label.clone(), Arc::new(WindowSession::new(path, started)));
    if let Err(error) = WebviewWindowBuilder::new(app, &label, WebviewUrl::default())
        .title("ShoruiPoppu")
        .inner_size(900.0, 700.0)
        .min_inner_size(360.0, 280.0)
        .build()
    {
        state.files.lock().unwrap().remove(&label);
        return Err(error);
    }
    Ok(())
}

fn open_requests(app: &tauri::AppHandle, paths: Vec<PathBuf>) {
    if paths.is_empty() {
        if let Err(error) = open_window(app, None) {
            eprintln!("Could not open window: {error}");
        }
    } else {
        for path in paths {
            if let Err(error) = open_window(app, Some(path)) {
                eprintln!("Could not open window: {error}");
            }
        }
    }
}

impl ViewerWindows {
    fn session(&self, label: &str) -> Result<Arc<WindowSession>, String> {
        self.files
            .lock()
            .map_err(|_| "Could not access the viewer state".to_string())?
            .get(label)
            .cloned()
            .ok_or_else(|| "Viewer window was not found".to_string())
    }
}

#[tauri::command]
async fn viewer_state(
    window: WebviewWindow,
    state: State<'_, ViewerWindows>,
) -> Result<Response, String> {
    let session = state.session(window.label())?;
    tauri::async_runtime::spawn_blocking(move || {
        Ok(session
            .document
            .lock()
            .map_err(|_| "Could not access the document")?
            .open())
    })
    .await
    .map_err(|_| "Could not open the document".to_string())?
}

#[tauri::command]
async fn worksheet(
    window: WebviewWindow,
    state: State<'_, ViewerWindows>,
    sheet: String,
) -> Result<Response, String> {
    let session = state.session(window.label())?;
    tauri::async_runtime::spawn_blocking(move || {
        session
            .document
            .lock()
            .map_err(|_| "Could not access the document")?
            .worksheet(&sheet)
    })
    .await
    .map_err(|_| "Could not load the worksheet".to_string())?
}

#[tauri::command]
async fn grid_cells(
    window: WebviewWindow,
    state: State<'_, ViewerWindows>,
    sheet: String,
    row: u32,
    rows: u32,
    column: u32,
    columns: u32,
) -> Result<workbook::GridSnapshot, String> {
    let session = state.session(window.label())?;
    tauri::async_runtime::spawn_blocking(move || {
        session
            .document
            .lock()
            .map_err(|_| "Could not access the document")?
            .viewport(&sheet, row, rows, column, columns)
    })
    .await
    .map_err(|_| "Could not load the grid cells".to_string())?
}

#[tauri::command]
async fn slide(
    window: WebviewWindow,
    state: State<'_, ViewerWindows>,
    index: usize,
) -> Result<Response, String> {
    let session = state.session(window.label())?;
    tauri::async_runtime::spawn_blocking(move || {
        session
            .document
            .lock()
            .map_err(|_| "Could not access the document")?
            .slide(index)
    })
    .await
    .map_err(|_| "Could not load the slide".to_string())?
}

#[tauri::command]
async fn slide_preview(
    window: WebviewWindow,
    state: State<'_, ViewerWindows>,
    index: usize,
) -> Result<office::Slide, String> {
    let session = state.session(window.label())?;
    tauri::async_runtime::spawn_blocking(move || {
        session
            .document
            .lock()
            .map_err(|_| "Could not access the document")?
            .slide_preview(index)
    })
    .await
    .map_err(|_| "Could not load the slide preview".to_string())?
}

#[tauri::command]
fn profile_mark(
    window: WebviewWindow,
    state: State<'_, ViewerWindows>,
    milestone: &str,
) -> Result<(), String> {
    if std::env::var_os("SHORUIPOPPU_PROFILE").is_none() {
        return Ok(());
    }
    let session = state.session(window.label())?;
    let flag = match milestone {
        "window" => &session.window_reported,
        "content" => &session.content_reported,
        _ => return Err("Unknown timing milestone.".into()),
    };
    if !flag.swap(true, Ordering::Relaxed) {
        eprintln!(
            "ShoruiPoppu timing: {} {milestone} after {} ms",
            window.label(),
            session.started.elapsed().as_millis()
        );
    }
    Ok(())
}

#[tauri::command]
fn open_file(window: WebviewWindow) {
    let app = window.app_handle().clone();
    window.dialog().file().pick_file(move |selection| {
        if let Some(path) = selection.and_then(|file| file.into_path().ok()) {
            if let Err(error) = open_window(&app, Some(path)) {
                eprintln!("Could not open window: {error}");
            }
        }
    });
}

fn main() {
    let mut args = std::env::args().skip(1);
    if args.next().as_deref() == Some("--internal-xls-reader") {
        let result = args
            .next()
            .ok_or_else(|| "Missing XLS source.".to_string())
            .and_then(|path| workbook::run_xls_reader(&path, args.next().as_deref()));
        if result.is_err() {
            std::process::exit(1);
        }
        return;
    }
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, args, cwd| {
            open_requests(app, paths_from_args(args, Path::new(&cwd)));
        }))
        .plugin(tauri_plugin_dialog::init())
        .manage(ViewerWindows::default())
        .invoke_handler(tauri::generate_handler![
            viewer_state,
            open_file,
            worksheet,
            grid_cells,
            profile_mark,
            slide,
            slide_preview
        ])
        .on_window_event(|window, event| {
            if matches!(event, WindowEvent::Destroyed) {
                window
                    .state::<ViewerWindows>()
                    .files
                    .lock()
                    .unwrap()
                    .remove(window.label());
            }
        })
        .setup(|app| {
            let cwd = std::env::current_dir().unwrap_or_default();
            open_requests(app.handle(), paths_from_args(std::env::args(), &cwd));
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running ShoruiPoppu");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_own_and_release_independent_sessions() {
        let windows = ViewerWindows::default();
        let first = Arc::new(WindowSession::new(None, Instant::now()));
        let second = Arc::new(WindowSession::new(None, Instant::now()));
        let weak = Arc::downgrade(&first);
        windows
            .files
            .lock()
            .unwrap()
            .insert("first".into(), first.clone());
        windows
            .files
            .lock()
            .unwrap()
            .insert("second".into(), second.clone());
        assert!(matches!(
            &*first.document.lock().unwrap().open().content,
            adapters::ViewerState::Empty
        ));
        windows.files.lock().unwrap().remove("first");
        drop(first);
        assert!(weak.upgrade().is_none());
        assert!(windows.session("first").is_err());
        assert!(windows.session("second").is_ok());
    }

    #[test]
    fn resolves_all_launch_paths_against_requesting_directory() {
        let args = ["app", "one file.md", "/tmp/two.pdf"].map(String::from);
        assert_eq!(
            paths_from_args(args, Path::new("/home/test")),
            [
                PathBuf::from("/home/test/one file.md"),
                PathBuf::from("/tmp/two.pdf")
            ]
        );
        assert!(paths_from_args(["app".into()], Path::new("/tmp")).is_empty());
    }
}
