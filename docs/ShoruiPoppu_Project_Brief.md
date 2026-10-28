# ShoruiPoppu — Project Brief

**Status:** Initial project brief  
**Date:** 28 October 2026  
**Product type:** Open-source desktop document viewer  
**Source host:** [GitHub](https://github.com/Igrise12/ShoruiPoppu)

## Overview

ShoruiPoppu is a read-only desktop app for opening documents quickly when a file is clicked. It displays the original file in a simple popup window and does not create converted document files. The project is planned as open-source software hosted on GitHub.

**Core principle:** click a file, open the popup, read the content.

## Problem

Opening a document just to view its contents can require launching a large office application or waiting for a document app to start. ShoruiPoppu provides a faster path for reading local files without editing features or a complicated interface.

## Target users

Desktop users who often need to inspect documents quickly. The initial implementation targets Linux, especially CachyOS/Hyprland. Windows support can follow once file opening and rendering for the primary formats are stable.

## Product goals

- Open documents from the file manager by clicking or double-clicking them.
- Show the viewer window promptly and prioritize the visible part of the document.
- Read the original file without creating a PDF or another converted copy.
- Support Markdown files as readable content in the viewer.
- Keep the UI minimal and focused on content.
- Process files locally without uploading them to an external service.
- Publish the source on GitHub as an open-source project.

## MVP scope

Initial formats:

- **PDF:** render PDF pages directly.
- **DOCX:** read the OOXML structure and render it directly in the document area.
- **XLS/XLSX:** read workbooks and display worksheets as scrollable grids.
- **PPTX:** read the OOXML presentation and render slides directly in a slideshow-style viewer.
- **Markdown (.md):** parse Markdown and render it directly in the viewer, without writing an intermediate HTML file.
- **CSV and plain text:** display as text or a simple grid.

The MVP is read-only. Legacy **PPT** files use a different binary format from PPTX and will need a separate reader; support them after a direct renderer is selected and validated. Other formats such as ODT, ODS, RTF, and legacy DOC can be added through their own format adapters.

## User experience

1. The user clicks a supported file.
2. The operating system opens ShoruiPoppu and passes the file path.
3. The popup appears promptly while parsing and rendering continue in the background.
4. The viewer displays the page, worksheet, slide, or Markdown content currently needed.
5. The user can scroll, search text, change zoom, navigate slides, open another file, or close the window.

To keep file opening fast, one application process can receive multiple file requests. Each file opens in a simple viewer window.

## Initial technical design

- **Desktop shell and backend:** Tauri 2 with Rust.
- **UI:** plain HTML, CSS, and TypeScript, without a large UI framework.
- **PDF:** PDF.js for page rendering.
- **XLS/XLSX/ODS:** Calamine for workbook reading; the UI grid virtualizes rows so a large workbook does not need to be rendered all at once.
- **DOCX:** direct rendering in the WebView, with an OOXML parser in the format adapter.
- **PPTX:** direct rendering of slides from the OOXML presentation package in the WebView.
- **Markdown:** a Rust Markdown parser renders headings, paragraphs, lists, links, code blocks, and tables directly in the viewer.
- **Desktop integration:** file/MIME associations, file paths from launch arguments, and single-instance handling.
- **Cache:** temporary in-memory cache for the open document; do not write converted PDF or HTML document files to disk.
- **Source control and collaboration:** Git and GitHub; publish the source repository publicly with the selected MIT license.

Each format uses its own adapter behind a consistent internal interface: detect the format, open the file, provide the visible page/sheet/slide/content, and close the document.

## Performance principles

- Show the window before parsing finishes.
- Render the visible page or slide first; render other content when requested.
- Virtualize spreadsheet rows.
- Keep the app process ready in the user session if profiling shows it is needed for fast popup display.
- Measure time to window display and first visible content on the target machine before setting numerical performance targets.

## Constraints and risks

- “No conversion” means the app does not create an intermediate document such as a PDF or HTML file. It still has to parse the file structure and render it for display.
- Complex DOCX layout may differ from Microsoft Word, particularly fonts, pagination, text boxes, SmartArt, and specialized elements.
- Complex PPTX content may differ from PowerPoint, particularly charts, SmartArt, embedded media, fonts, transitions, and animations. The first version focuses on static slide viewing.
- Markdown rendering must handle untrusted content safely; raw embedded HTML and external resources should be restricted or explicitly controlled.
- Legacy PPT and DOC files need separate readers; they are not covered by the PPTX and DOCX parsers.
- Large workbooks, image-heavy PDFs, encrypted documents, and damaged files need dedicated handling.
- Test each format against representative real-world files before claiming full compatibility.

## Out of scope for the MVP

- Editing or saving documents.
- Collaboration, cloud sync, or document uploads.
- Reimplementing all Microsoft Office behavior.
- A feature-heavy interface like a full office editor.

## Acceptance criteria

- Clicking a supported file opens a ShoruiPoppu popup.
- If the app is already running, new file requests are sent to its existing process.
- No converted PDF or HTML document file is created as an intermediate artifact.
- PDF, DOCX, XLS/XLSX, PPTX, Markdown (.md), CSV, and plain text open within the MVP scope.
- Markdown headings, paragraphs, lists, links, code blocks, and tables are readable in the viewer.
- Large workbooks and PDFs can begin displaying without rendering all content up front.
- The UI remains simple, read-only, and content-focused.
- Files stay on the user's device and are not sent over the network.
- PPTX opens on a slide, with basic slide navigation; animations and transitions are outside the first version.
- The project source is available in a public GitHub repository under a selected open-source license before its first public release.

## Implementation phases

1. Create the public GitHub repository, select an open-source license, and add a README with setup and contribution instructions.
2. Build the Tauri shell, minimal viewer window, file-argument handling, and Linux file associations.
3. Implement direct PDF rendering and Markdown rendering.
4. Implement XLS/XLSX as a virtualized grid.
5. Implement direct DOCX rendering and test layout variations.
6. Implement PPTX slide rendering and basic slide navigation.
7. Add legacy PPT and other formats based on actual need and compatibility testing.
