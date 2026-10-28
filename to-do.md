# ShoruiPoppu To-Do

Implementation checklist based on the [project brief](docs/ShoruiPoppu_Project_Brief.md), [architecture](docs/architecture.md), and [tech stack](docs/tech-stack.md).

## 1. Project setup

- [x] Create the public GitHub repository.
- [x] Choose an open-source license and add its license file.
- [x] Add a README with project goals, supported formats, setup, and contribution instructions.
- [x] Set up the Tauri 2 application with a Rust backend and a plain HTML/CSS/TypeScript frontend.

**Progress note:** Initial PDF, Markdown, workbook, DOCX, PPTX, CSV, and text adapters and viewer controls have been implemented. PDF page selection, sticky grid row headers, and basic PPTX layout/master inheritance have been added. The packaged launcher has passed a multiple-file GIO forwarding check on Hyprland. The unchecked items below remain open until visual desktop, target file-manager, representative-file, reliability, and performance verification confirms the intended behavior; the per-window adapter lifecycle and explicit capability messages have been implemented and tested. See [release-readiness notes](docs/release-readiness.md) for task 8 evidence and outstanding checks.

## 2. Desktop shell and shared viewer

- [x] Create the minimal, read-only viewer window and loading, error, and unsupported-format states.
- [x] Read file paths passed at launch and route files by detected format.
- [x] Add single-instance handling so new file requests reach the running process.
- [x] Open each requested file in its own viewer window.
- [ ] Add Linux file/MIME associations and verify opening files from the target file manager.
- [x] Define the shared format-adapter lifecycle and Rust-to-TypeScript content and error messages.
- [ ] Add shared viewer controls for scrolling, search where supported, zoom where applicable, opening another file, and closing the window.
- [x] Keep source files read-only and local; use only temporary in-memory document state and create no converted document files.

## 3. PDF and Markdown

- [x] Integrate PDF.js for direct PDF page rendering.
- [ ] Show the window before parsing completes and render the visible PDF page first.
- [ ] Add PDF scrolling, search, and zoom controls where supported by the viewer.
- [x] Choose a Rust Markdown parser and define the safe parsed-content representation.
- [ ] Render Markdown headings, paragraphs, lists, links, code blocks, and tables in the viewer.
- [ ] Restrict raw embedded HTML and external resources in Markdown content.

## 4. XLS and XLSX

- [x] Integrate Calamine to read XLS/XLSX workbooks and worksheet data.
- [ ] Display worksheets in a scrollable grid with row virtualization.
- [ ] Verify a large workbook can show useful visible cells without rendering the full worksheet.

## 5. DOCX

- [ ] Select and validate an OOXML parsing approach for DOCX.
- [x] Add a DOCX adapter that reads the source package and supplies structured content to the WebView.
- [ ] Render DOCX content directly in the viewer without creating an intermediate document file.
- [ ] Review representative files for layout differences, including fonts, pagination, text boxes, and specialized elements.

## 6. PPTX

- [ ] Select and validate an OOXML parsing approach for PPTX.
- [x] Add a PPTX adapter that reads slide content and supplies it to the WebView.
- [ ] Render static slides directly and add basic slide navigation.
- [ ] Review representative presentations, including charts, SmartArt, embedded media, fonts, transitions, and animations; document unsupported or limited content.

## 7. CSV, plain text, and reliability

- [ ] Add CSV viewing as text or a simple grid.
- [ ] Add plain-text viewing.
- [ ] Handle unsupported, malformed, damaged, and encrypted files with clear errors.
- [ ] Check large and image-heavy PDFs and large workbooks for useful first-content display and responsive navigation.
- [ ] Confirm documents stay on-device and no intermediate PDF, HTML, or converted document is written to disk.
- [ ] Measure time to window display and first visible content on the target Linux machine; set performance targets from those measurements.
- [ ] Consider keeping the process ready in the user session only if profiling shows it is needed.

## 8. MVP release readiness

- [ ] Verify file-manager opening and single-instance forwarding on the target Linux desktop.
- [ ] Check the MVP format set: PDF, DOCX, XLS/XLSX, PPTX, Markdown, CSV, and plain text.
- [ ] Confirm the viewer remains read-only, minimal, and content-focused.
- [x] Confirm the public repository has a selected license and setup/contribution documentation before the first public release.

## Later, based on need

- [ ] Evaluate separate readers for legacy PPT and DOC.
- [ ] Evaluate adapters for ODT, ODS, RTF, and other requested formats.
- [ ] Consider Windows support after primary file opening and rendering are stable on Linux.
