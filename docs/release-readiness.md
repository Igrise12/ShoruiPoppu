# MVP release-readiness notes

## Evidence from 28 October 2026

- The release build produced a Debian bundle containing the executable and a desktop entry with all eight MVP MIME associations. A previous Hyprland check verified multiple-file GIO forwarding, including paths with spaces and non-Latin characters.
- `npm run build`, `npm run test:pdf`, `npm run test:navigation`, `cargo check --manifest-path src-tauri/Cargo.toml`, and `cargo test --manifest-path src-tauri/Cargo.toml` passed. The PDF test must run after the build; simultaneous builds can replace its `dist` assets while it is running.
- The local checkout has an MIT license and setup and contribution instructions. The GitHub repository is public but was empty when checked, so these files are not yet published.
- This session could not access native application windows. Dolphin and Nautilus Open With, visual rendering and navigation, and runtime file and network activity were not verified in this pass. Existing automated tests do not establish those results.

## Remaining Hyprland checks

Use synthetic files only. The repository supplies PDF, DOCX, XLS, and PPTX samples in `src-tauri/tests/fixtures/`. Generate a small XLSX with `python3 scripts/make_benchmark_workbook.py /tmp/shorui-check.xlsx --rows 20`; create small Markdown, CSV, and UTF-8 text examples, plus damaged and unsupported examples, in a new temporary directory. Copy examples to names with spaces and non-Latin characters. Record hashes before and after testing to check that sources did not change.

1. Register the built application as an Open With option using the README instructions, without setting it as a default. In both Dolphin and Nautilus, open a sample using Open With, then open a second sample while the first window is present. Confirm both windows display the intended files and belong to the same main application process. Repeat with the spaced and non-Latin filenames. Check the default applications are unchanged.
2. Open each MVP format and inspect visible content. Navigate both PDF pages, both worksheets in `workbook.xls`, and the slides in `example.pptx`. Check the generated XLSX and the Markdown, CSV, and text examples. Confirm damaged and unsupported files show clear errors. Check that the interface remains focused on content and has no editing or saving controls.
3. Compare the test-file hashes after closing the viewer. Observe the application and its child processes' file and network activity while opening and navigating the samples, including any writes outside the sample directory and outbound connections. Distinguish normal runtime files from converted document content. Record the observation method and findings; unchanged source hashes alone do not prove that no copies or uploads occurred.
4. Before release, verify that the public repository contains the MIT license and the setup and contribution instructions. Record the tested build and results, and check off task 8 only after each corresponding check passes.

These checks do not replace representative real-world Office fidelity reviews or large-document performance testing.
