# ShoruiPoppu Tech Stack

**Status:** Initial stack, based on the project brief  
**Date:** 28 October 2026

## Selected technologies

| Area | Technology | Role and notes |
| --- | --- | --- |
| Desktop application | Tauri 2 | Native desktop shell, application lifecycle, windows, and operating-system integration. |
| Backend | Rust | File handling, format adapters, and the Rust-based Markdown parser. |
| Frontend | HTML, CSS, TypeScript | Minimal viewer UI rendered in the Tauri WebView. No large UI framework is planned. |
| PDF | PDF.js | Read and render PDF pages directly, prioritizing the visible page. |
| XLS/XLSX workbooks | Calamine | Read workbook and worksheet data. The UI presents it in a virtualized, scrollable grid. |
| DOCX | OOXML parsing plus WebView rendering | Read the OOXML package and render document content directly in the viewer. The initial subset parser uses Rust ZIP and XML libraries; representative-file validation is pending. |
| PPTX | OOXML parsing plus WebView rendering | Read presentation content and render static slides directly, with basic navigation. The initial subset parser uses Rust ZIP and XML libraries; representative-file validation is pending. |
| Markdown | Rust Markdown parser | Parse and display headings, paragraphs, lists, links, code blocks, and tables without writing an intermediate HTML file. The initial parser uses pulldown-cmark events. |
| CSV and plain text | Local reader in the format-adapter layer | Display as text or a simple grid. The initial adapter uses the Rust UTF-8 reader and csv crate. |
| Desktop integration | Tauri and platform integration | File/MIME associations, launch-path handling, and single-instance forwarding. Initial focus is Linux, especially CachyOS/Hyprland. |
| Source control and hosting | Git and GitHub | Public repository: [Igrise12/ShoruiPoppu](https://github.com/Igrise12/ShoruiPoppu). The project uses the MIT license. |

## Document handling rules

- Read the original file directly and keep processing on the user's device.
- Do not create an intermediate PDF, HTML file, or converted document copy.
- Use temporary in-memory state for an open document; do not persist a converted-document cache.
- Treat Markdown as untrusted input. Raw embedded HTML and external resources must be restricted or explicitly controlled.
- Keep the MVP read-only; do not write changes back to source files.

## Rendering and performance

- Show the Tauri window before parsing and rendering finish.
- Load and render the visible PDF page or PPTX slide first, then continue on demand.
- Virtualize spreadsheet rows to keep large worksheets responsive.
- Use PDF.js for PDF page rendering and the WebView for direct DOCX, PPTX, and Markdown display.
- Profile on the target machine before setting numerical startup or first-render targets. A resident process is only warranted if measurements show it is needed.

## Scope notes

The MVP formats are PDF, DOCX, XLS/XLSX, PPTX, Markdown (`.md`), CSV, and plain text. Calamine is identified for XLS/XLSX workbook reading. Although Calamine may also be useful for ODS, ODS is listed as a possible later format in the brief and is not an MVP acceptance requirement.

Legacy PPT and DOC require separate readers because their binary formats differ from PPTX and DOCX. ODT and other non-MVP formats can be added through separate adapters after a reader is selected and validated.

## Decisions still needed

- Validate the initial ZIP/XML DOCX and PPTX implementation against representative files and address fidelity gaps.
- Validate Markdown rendering and safety end to end in the desktop WebView.
- Measure first-content time and memory use for representative large workbooks on the target desktop.
- Confirm packaging and file-association details for the target Linux desktop; choose additional platform-specific approaches when Windows support begins.
