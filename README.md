# ShoruiPoppu

ShoruiPoppu is an early-stage, open-source desktop document viewer. Its goal is to open local files quickly in a simple, read-only window, without uploading them or writing converted document copies. Linux, particularly CachyOS with Hyprland, is the initial target.

The desktop shell opens one window per requested file. Early viewers are implemented for PDF, DOCX, XLS/XLSX, PPTX, Markdown, CSV, and UTF-8 plain text. Office rendering is incomplete, and desktop and privacy checks are still in progress. See the [to-do list](to-do.md) and [release-readiness notes](docs/release-readiness.md) for status.

## Requirements

The initial target is Linux, especially CachyOS with Hyprland. Other Linux desktops may work but have not been fully verified; Windows and macOS are not currently supported targets. To build from source, install Git, Node.js 22.13 or newer, npm, Rust and Cargo, and the Linux system dependencies in the [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/#linux). The first build downloads dependencies and may take some time.

On Arch Linux or CachyOS, install the Tauri system packages with:

```sh
sudo pacman -S --needed webkit2gtk-4.1 base-devel curl wget file openssl appmenu-gtk-module libappindicator-gtk3 librsvg xdotool desktop-file-utils
```

Install Node.js and Rust using your preferred package manager or their official installers. Debian and Ubuntu users should follow the Tauri prerequisites for their distribution.

## Build and install

There is no published prebuilt release yet. Clone the repository and build the application locally:

```sh
git clone https://github.com/Igrise12/ShoruiPoppu.git
cd ShoruiPoppu
npm ci
npm run tauri build
```

On Debian-based systems, the build creates a package in `src-tauri/target/release/bundle/deb/`. Install it with:

```sh
sudo apt install ./src-tauri/target/release/bundle/deb/ShoruiPoppu_0.1.0_amd64.deb
```

The filename depends on the version and architecture; use the filename produced by your build. To uninstall the Debian package, run `sudo apt remove shorui-poppu`.

On Arch Linux or CachyOS, install the release executable and register a user-local desktop entry after building:

```sh
install -Dm755 src-tauri/target/release/shoruipoppu "$HOME/.local/bin/shoruipoppu"
mkdir -p "$HOME/.local/share/applications"
cat > "$HOME/.local/share/applications/shoruipoppu.desktop" <<EOF
[Desktop Entry]
Name=ShoruiPoppu
Comment=A local, read-only document viewer
Exec="$HOME/.local/bin/shoruipoppu" %F
Terminal=false
Type=Application
MimeType=application/pdf;application/vnd.openxmlformats-officedocument.wordprocessingml.document;application/vnd.ms-excel;application/vnd.openxmlformats-officedocument.spreadsheetml.sheet;application/vnd.openxmlformats-officedocument.presentationml.presentation;text/markdown;text/csv;text/plain;
EOF
update-desktop-database "$HOME/.local/share/applications"
```

This only registers ShoruiPoppu as an option in **Open With**; it does not change your default applications. To remove this user-local installation, delete `~/.local/bin/shoruipoppu` and `~/.local/share/applications/shoruipoppu.desktop`, then run `update-desktop-database ~/.local/share/applications`.

## Open a document

Use **Open With** in your file manager and choose ShoruiPoppu, or run the built or installed executable with one or more local file paths:

```sh
~/.local/bin/shoruipoppu "/path/to/document.pdf" "/path/to/workbook.xlsx"
```

For a Debian package installation, use `shoruipoppu` instead of `~/.local/bin/shoruipoppu`. Running it without a path opens an empty viewer. Each file gets its own window; further requests are forwarded to the running application. Ctrl+O opens another file and the document area can be focused for keyboard scrolling. Available search, zoom, page, worksheet, and slide controls depend on the format.

## Troubleshooting

If the window does not appear and the terminal reports graphics-buffer errors on Wayland, try launching it with WebKit's DMA-BUF renderer disabled for that run:

```sh
WEBKIT_DISABLE_DMABUF_RENDERER=1 ./src-tauri/target/release/shoruipoppu
```

This setting has helped on the tested Hyprland desktop, but may not resolve every Wayland protocol error. It applies only to that command, not file-manager launches.

To check file-manager integration on Linux, use **Open With** to select ShoruiPoppu for an example file, then open another file while the first window remains open. Each request should create a separate window in the same process. The packaged desktop entry has been validated and tested with GIO forwarding multiple local files on Hyprland, including names with spaces and non-Latin characters. The Open With flows in Dolphin and Nautilus still need verification; see the [release-readiness notes](docs/release-readiness.md).

## Format support and limits

- PDF pages are rendered on demand; use the page number or previous/next buttons to navigate. Search locates a matching page and zoom keeps the current page in view. Ctrl+F focuses search and Ctrl+Plus/Minus changes zoom. Password-protected PDFs are not supported.
- Markdown raw HTML and external resources are disabled. Links and image descriptions are displayed as text rather than opened or fetched.
- XLS/XLSX and CSV use a virtualized grid. XLSX cells are streamed into a sparse in-memory sheet; only visible workbook cells are sent to the WebView. The first populated cells appear while the rest of the selected XLSX sheet is scanned in the background; its scrollable bounds expand as cells are discovered. If a sheet exceeds a limit or is damaged, already-read cells remain visible with an error. XLS parsing uses a separate Linux helper because its parser loads whole workbooks eagerly; the helper has a 512 MiB address-space limit and 30 seconds of CPU time. Large or sparse XLS files can therefore be rejected. Large-workbook desktop performance has not yet been validated.
- DOCX supports paragraphs, selected heading styles, common run formatting, simple decimal and bullet lists, simple tables, and sized embedded raster images. Pagination, font substitution, advanced numbering, text boxes, complex elements, and layout can differ or be omitted.
- PPTX supports positioned and commonly styled text, basic shapes, RGB and base theme colors, grouped-shape positioning, rotation, embedded raster images, and basic layout/master artwork and solid backgrounds. Slide placeholders can inherit their position from their layout. Slide navigation supports arrow keys, Page Up/Down, Home, and End. Complex master layouts, inherited text formatting, theme effects, group rotation, charts, SmartArt, embedded media, transitions, animation, and precise text layout can differ or be omitted.
- Individual source files are limited to 100 MB; XLSX packages are limited to 128 MiB uncompressed and 64 MiB per part, and a selected worksheet to one million nonempty cells and 64 MiB of cell text. Office package parts are limited to 16 MB and embedded images to 32 MB per document or slide. Unsupported or over-limit images can be omitted. Other encodings than UTF-8 are not supported for text and CSV.
- No network resources are intentionally loaded from document content and no converted documents are written. These guarantees still need end-to-end verification on the target Linux desktop.

## Measuring startup

To log local timing estimates for a built application, run:

```sh
SHORUIPOPPU_PROFILE=1 src-tauri/target/release/shoruipoppu /path/to/document.pdf
```

To generate a synthetic workbook for a repeatable local check, run `python3 scripts/make_benchmark_workbook.py /tmp/shorui-benchmark.xlsx` and open that file with profiling enabled. The command requires a new output path and does not read your documents. See [performance notes](docs/performance.md) for an example comparison.

The terminal reports the first viewer frame and the first content frame in milliseconds. The first window is timed from application initialization; later windows are timed from their opening request. These are approximate frame timings, not external measurements of compositor presentation. They contain no document names or content. Collect measurements on the target machine before setting performance targets.

## Contributing

Issues and pull requests are welcome. Discuss substantial changes in an issue first, keep changes aligned with the [architecture](docs/architecture.md), and include a description of what changed and how you checked it. After installing the requirements and running `npm ci`, start a development build with `npm run tauri dev`.

Before opening a pull request, run these checks from the project root:

```sh
npm run build
npm run test:pdf
npm run test:navigation
cargo check --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml
```

Run the frontend build before the PDF test, and check the desktop app when a change affects it. Test documents in `src-tauri/tests/fixtures/` contain synthetic content. Please do not include private documents in issues or pull requests.

## License

[MIT](LICENSE).
