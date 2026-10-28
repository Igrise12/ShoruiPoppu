# Agent instructions

Read `to-do.md` and the relevant project documents in `docs/` before changing product behavior. The initial target is Linux; the MVP is a local, read-only viewer. Do not upload document content, modify source documents, or write intermediate converted documents. Treat document content as untrusted.

The frontend is plain HTML, CSS, and TypeScript; the desktop backend is Rust with Tauri 2. Keep format-specific parsing behind the adapter boundary described in `docs/architecture.md`.

Run `npm run build` for frontend changes and `cargo check --manifest-path src-tauri/Cargo.toml` for Rust or Tauri changes. Verify desktop behavior when the change affects the application window or integration.

<!-- CODEGRAPH_START -->
## CodeGraph

If `.codegraph/` exists at the repository root, use `codegraph_explore` (or `codegraph explore "<symbol names or question>"`) before grep, find, or reading files when locating or understanding code. Skip CodeGraph if the directory is absent.
<!-- CODEGRAPH_END -->
