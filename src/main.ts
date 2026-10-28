import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { pdfPage, slideTarget } from "./navigation";
import type { RenderTask } from "pdfjs-dist/legacy/build/pdf.mjs";
import workerUrl from "pdfjs-dist/legacy/build/pdf.worker.min.mjs?url";
import "./style.css";

type Token = { kind: string; text: string };
type TextStyle = { bold: boolean; italic: boolean; underline: boolean; size_pt: number | null; color: string | null; font: string | null; align: string | null; list_level: number | null; marker: string | null; width_px: number | null; height_px: number | null };
type Block = { kind: string; text: string; image: string | null; children: Block[]; style: TextStyle | null };
type SlideRun = { text: string; style: TextStyle };
type SlideParagraph = { align: string | null; marker: string | null; level: number; runs: SlideRun[] };
type SlideItem = { kind: string; text: string; image: string | null; fill: string | null; outline: string | null; geometry: string; rotation: number; flip_h: boolean; flip_v: boolean; paragraphs: SlideParagraph[]; x: number; y: number; width: number; height: number };
type Slide = { width: number; height: number; background: string | null; items: SlideItem[] };
type ViewerState =
  | { state: "empty" }
  | { state: "pdf"; name: string; bytes: number[] }
  | { state: "markdown"; name: string; tokens: Token[] }
  | { state: "text"; name: string; text: string }
  | { state: "grid"; name: string; sheets: string[]; sheet: string; rows: string[][]; row_offset: number; column_offset: number }
  | { state: "workbook"; name: string; sheets: string[]; sheet: string; row_offset: number; column_offset: number; row_count: number; column_count: number; complete: boolean }
  | { state: "docx"; name: string; blocks: Block[] }
  | { state: "pptx"; name: string; index: number; count: number; slide: Slide }
  | { state: "unsupported"; name: string }
  | { state: "error"; name: string; message: string };

type Capabilities = { search: boolean; zoom: boolean; worksheets: boolean; slides: boolean };
type Response = { content: ViewerState; capabilities: Capabilities };
type GridCell = { row: number; column: number; value: string };
type GridSnapshot = { cells: GridCell[]; row_offset: number; column_offset: number; row_count: number; column_count: number; complete: boolean; error: string | null };

const app = document.querySelector<HTMLElement>("#app")!;
app.innerHTML = `<header class="toolbar"><div class="brand"><span class="mark" aria-hidden="true">S</span><span>ShoruiPoppu</span></div><div class="actions"><span id="filename"></span><button id="open" type="button">Open file</button><button id="close" class="close" type="button" aria-label="Close window" title="Close window">×</button></div></header><div id="controls" class="controls" hidden></div><main class="viewer" tabindex="0" aria-label="Document content" aria-live="polite" aria-busy="true"><div class="message"><h1>Opening file…</h1><p>Please wait.</p></div></main>`;
const viewer = app.querySelector<HTMLElement>(".viewer")!;
const controls = app.querySelector<HTMLElement>("#controls")!;
const filename = app.querySelector<HTMLElement>("#filename")!;
let cleanup = () => {};
let contentReported = false;
function afterPaint(milestone: "window" | "content") {
  requestAnimationFrame(() => requestAnimationFrame(() => { void invoke("profile_mark", { milestone }).catch(() => {}); }));
}
function firstContent() { if (!contentReported) { contentReported = true; afterPaint("content"); } }
afterPaint("window");
function message(title: string, detail: string) {
  viewer.className = "viewer centered";
  const box = document.createElement("div"); box.className = "message";
  const h = document.createElement("h1"); h.textContent = title;
  const p = document.createElement("p"); p.textContent = detail;
  box.append(h, p); viewer.replaceChildren(box);
}
function button(label: string, click: () => void): HTMLButtonElement {
  const el = document.createElement("button"); el.type = "button"; el.textContent = label; el.addEventListener("click", click); return el;
}
function applyTextStyle(element: HTMLElement, style: TextStyle | null, slideWidth?: number) {
  if (!style) return;
  if (style.bold) element.style.fontWeight = "bold";
  if (style.italic) element.style.fontStyle = "italic";
  if (style.underline) element.style.textDecoration = "underline";
  if (style.color) element.style.color = style.color;
  if (style.font) element.style.fontFamily = `"${style.font}", sans-serif`;
  if (style.size_pt) element.style.fontSize = slideWidth ? `${style.size_pt * 12700 / slideWidth * 100}cqw` : `${style.size_pt}pt`;
  if (style.align && ["left", "right", "center", "justify"].includes(style.align)) element.style.textAlign = style.align;
  if (style.list_level !== null) element.style.marginLeft = `${(style.list_level + 1) * 1.5}em`;
  if (style.width_px) element.style.width = `${style.width_px}px`;
  if (element instanceof HTMLImageElement && style.width_px && style.height_px) { element.style.aspectRatio = `${style.width_px} / ${style.height_px}`; element.style.height = "auto"; }
  else if (style.height_px) element.style.height = `${style.height_px}px`;
}
function renderGrid(state: Extract<ViewerState, { state: "grid" }>) {
  if (state.sheets.length) {
    const select = document.createElement("select"); select.setAttribute("aria-label", "Worksheet");
    for (const sheet of state.sheets) { const option = document.createElement("option"); option.value = sheet; option.textContent = sheet; option.selected = sheet === state.sheet; select.append(option); }
    select.addEventListener("change", async () => {
      select.disabled = true;
      try { await render(await invoke<Response>("worksheet", { sheet: select.value })); }
      catch { message("Could not open worksheet", "The worksheet could not be read."); }
      finally { select.disabled = false; }
    });
    controls.append(select); controls.hidden = false;
  }
  viewer.className = "viewer grid-view";
  const canvas = document.createElement("div"); canvas.className = "grid-canvas";
  canvas.style.height = `${(state.rows.length + 1) * 32}px`;
  const maxColumns = state.rows.reduce((max, row) => Math.max(max, row.length), 0);
  canvas.style.width = `${Math.max(1, maxColumns) * 160 + 56}px`;
  viewer.replaceChildren(canvas);
  const draw = () => {
    const start = Math.max(0, Math.floor((viewer.scrollTop - 32) / 32) - 5);
    const end = Math.min(state.rows.length, Math.ceil((viewer.scrollTop + viewer.clientHeight - 32) / 32) + 5);
    const firstColumn = Math.max(0, Math.floor((viewer.scrollLeft - 56) / 160) - 2);
    const lastColumn = Math.ceil((viewer.scrollLeft + viewer.clientWidth - 56) / 160) + 2;
    const fragment = document.createDocumentFragment();
    const header = document.createElement("div"); header.className = "grid-row grid-header"; header.style.top = `${viewer.scrollTop}px`;
    const corner = document.createElement("span"); corner.className = "row-number"; corner.style.left = `${viewer.scrollLeft}px`; header.append(corner);
    const columnName = (number: number) => { let result = ""; for (number++; number > 0; number = Math.floor((number - 1) / 26)) result = String.fromCharCode(65 + (number - 1) % 26) + result; return result; };
    for (let column = firstColumn; column < Math.min(lastColumn, maxColumns); column++) {
      const cell = document.createElement("span"); cell.className = "grid-cell"; cell.style.left = `${56 + column * 160}px`; cell.textContent = columnName(column + state.column_offset); header.append(cell);
    }
    fragment.append(header);
    for (let i = start; i < end; i++) {
      const row = document.createElement("div"); row.className = "grid-row"; row.style.top = `${(i + 1) * 32}px`;
      const index = document.createElement("span"); index.className = "row-number"; index.style.left = `${viewer.scrollLeft}px`; index.textContent = String(i + 1 + state.row_offset); row.append(index);
      for (let column = firstColumn; column < Math.min(lastColumn, state.rows[i].length); column++) {
        const value = state.rows[i][column]; const cell = document.createElement("span"); cell.className = "grid-cell"; cell.style.left = `${56 + column * 160}px`; cell.textContent = value; cell.title = value; row.append(cell);
      }
      fragment.append(row);
    }
    canvas.replaceChildren(fragment);
  };
  viewer.addEventListener("scroll", draw); cleanup = () => viewer.removeEventListener("scroll", draw); draw();
}
function renderWorkbook(state: Extract<ViewerState, { state: "workbook" }>, capabilities: Capabilities) {
  const select = document.createElement("select"); select.setAttribute("aria-label", "Worksheet");
  for (const sheet of state.sheets) { const option = document.createElement("option"); option.value = sheet; option.textContent = sheet; option.selected = sheet === state.sheet; select.append(option); }
  if (capabilities.worksheets) {
    select.addEventListener("change", async () => {
      select.disabled = true;
      try { await render(await invoke<Response>("worksheet", { sheet: select.value })); viewer.scrollTop = 0; viewer.scrollLeft = 0; }
      catch (error) { status.textContent = String(error); select.value = state.sheet; }
      finally { select.disabled = false; }
    });
    controls.append(select); controls.hidden = false;
  }
  const status = document.createElement("span"); status.className = "status"; status.setAttribute("aria-live", "polite"); controls.append(status);
  viewer.className = "viewer grid-view";
  const canvas = document.createElement("div"); canvas.className = "grid-canvas";
  canvas.style.height = `${(state.row_count + 1) * 32}px`;
  canvas.style.width = `${Math.max(1, state.column_count) * 160 + 56}px`;
  viewer.replaceChildren(canvas);
  let requestId = 0; let timer: ReturnType<typeof setTimeout> | undefined; let scanTimer: ReturnType<typeof setTimeout> | undefined; let disposed = false;
  const columnName = (number: number) => { let result = ""; for (number++; number > 0; number = Math.floor((number - 1) / 26)) result = String.fromCharCode(65 + (number - 1) % 26) + result; return result; };
  const draw = async () => {
    const id = ++requestId;
    const row = Math.max(0, Math.floor((viewer.scrollTop - 32) / 32) - 4);
    const rows = Math.min(128, Math.max(0, Math.ceil((viewer.scrollTop + viewer.clientHeight - 32) / 32) + 4 - row), Math.max(0, state.row_count - row));
    const column = Math.max(0, Math.floor((viewer.scrollLeft - 56) / 160) - 2);
    const columns = Math.min(64, Math.max(0, Math.ceil((viewer.scrollLeft + viewer.clientWidth - 56) / 160) + 2 - column), Math.max(0, state.column_count - column));
    try {
      const snapshot = await invoke<GridSnapshot>("grid_cells", { sheet: state.sheet, row, rows, column, columns });
      if (disposed || id !== requestId) return;
      const offsetChanged = state.row_offset !== snapshot.row_offset || state.column_offset !== snapshot.column_offset;
      const boundsChanged = offsetChanged || state.row_count !== snapshot.row_count || state.column_count !== snapshot.column_count;
      if (offsetChanged) {
        if (viewer.scrollTop > 0) viewer.scrollTop += (state.row_offset - snapshot.row_offset) * 32;
        if (viewer.scrollLeft > 0) viewer.scrollLeft += (state.column_offset - snapshot.column_offset) * 160;
      }
      state.row_offset = snapshot.row_offset; state.column_offset = snapshot.column_offset;
      state.row_count = snapshot.row_count; state.column_count = snapshot.column_count; state.complete = snapshot.complete;
      canvas.style.height = `${(state.row_count + 1) * 32}px`;
      canvas.style.width = `${Math.max(1, state.column_count) * 160 + 56}px`;
      if (offsetChanged) { schedule(); return; }
      const values = new Map(snapshot.cells.map(cell => [`${cell.row}:${cell.column}`, cell.value]));
      const fragment = document.createDocumentFragment();
      const header = document.createElement("div"); header.className = "grid-row grid-header"; header.style.top = `${viewer.scrollTop}px`;
      const corner = document.createElement("span"); corner.className = "row-number"; corner.style.left = `${viewer.scrollLeft}px`; header.append(corner);
      for (let c = column; c < column + columns; c++) { const cell = document.createElement("span"); cell.className = "grid-cell"; cell.style.left = `${56 + c * 160}px`; cell.textContent = columnName(c + state.column_offset); header.append(cell); }
      fragment.append(header);
      for (let r = row; r < row + rows; r++) {
        const line = document.createElement("div"); line.className = "grid-row"; line.style.top = `${(r + 1) * 32}px`;
        const index = document.createElement("span"); index.className = "row-number"; index.style.left = `${viewer.scrollLeft}px`; index.textContent = String(r + 1 + state.row_offset); line.append(index);
        for (let c = column; c < column + columns; c++) { const value = values.get(`${r}:${c}`) ?? ""; const cell = document.createElement("span"); cell.className = "grid-cell"; cell.style.left = `${56 + c * 160}px`; cell.textContent = value; cell.title = value; line.append(cell); }
        fragment.append(line);
      }
      canvas.replaceChildren(fragment);
      status.textContent = snapshot.error ?? (snapshot.complete ? "" : "Loading worksheet…");
      if (snapshot.cells.length || (snapshot.complete && !snapshot.error)) firstContent();
      clearTimeout(scanTimer); if (!snapshot.complete) scanTimer = setTimeout(schedule, 150); else if (boundsChanged) schedule();
    } catch (error) { if (!disposed && id === requestId) status.textContent = `Could not load cells: ${String(error)}`; }
  };
  const schedule = () => { requestId++; clearTimeout(timer); timer = setTimeout(() => void draw(), 30); };
  viewer.addEventListener("scroll", schedule);
  const resize = new ResizeObserver(schedule); resize.observe(viewer);
  cleanup = () => { disposed = true; requestId++; clearTimeout(timer); clearTimeout(scanTimer); resize.disconnect(); viewer.removeEventListener("scroll", schedule); };
  void draw();
}
async function renderPdf(state: Extract<ViewerState, { state: "pdf" }>, capabilities: Capabilities) {
  viewer.className = "viewer pdf-view";
  const pdfjs = await import("pdfjs-dist/legacy/build/pdf.mjs");
  pdfjs.GlobalWorkerOptions.workerSrc = workerUrl;
  const pdfAssets = new URL("pdfjs/", window.location.href);
  const task = pdfjs.getDocument({ data: new Uint8Array(state.bytes), disableAutoFetch: true, disableStream: true,
    standardFontDataUrl: new URL("standard_fonts/", pdfAssets).href,
    cMapUrl: new URL("cmaps/", pdfAssets).href, cMapPacked: true,
    wasmUrl: new URL("wasm/", pdfAssets).href });
  let pdf: Awaited<typeof task.promise>;
  try { pdf = await task.promise; } catch { message("Could not open PDF", "The PDF is damaged, encrypted, or cannot be read."); return; }
  let scale = 1.25;
  let observer: IntersectionObserver | undefined;
  let renderGeneration = 0;
  let nextLoad = 0;
  let pendingAnchor: { page: number; fraction: number; left: number } | undefined;
  const rendering = new Map<number, RenderTask>();
  const pages = document.createElement("div"); pages.className = "pdf-pages"; viewer.replaceChildren(pages);
  let holders: HTMLElement[] = [];
  const firstVisiblePage = () => {
    const top = viewer.getBoundingClientRect().top;
    let low = 0; let high = holders.length;
    while (low < high) { const middle = Math.floor((low + high) / 2); if (holders[middle].getBoundingClientRect().bottom <= top) low = middle + 1; else high = middle; }
    return holders[low];
  };
  const drawPages = async (anchor?: { page: number; fraction: number; left: number }) => {
    const generation = ++renderGeneration;
    observer?.disconnect(); for (const task of rendering.values()) task.cancel(); rendering.clear(); pages.replaceChildren(); holders = [];
    const first = await pdf.getPage(1); if (generation !== renderGeneration) return;
    const size = first.getViewport({ scale });
    observer = new IntersectionObserver(entries => {
      for (const entry of entries) {
        const holder = entry.target as HTMLElement; const number = Number(holder.dataset.page);
        if (!entry.isIntersecting) {
          holder.dataset.visible = "false"; rendering.get(number)?.cancel(); rendering.delete(number); holder.replaceChildren(); delete holder.dataset.loading; continue;
        }
        holder.dataset.visible = "true";
        if (holder.dataset.loading) continue;
        const loadId = String(++nextLoad); holder.dataset.loading = loadId;
        void (async () => {
          try {
            const page = await pdf.getPage(number);
            if (generation !== renderGeneration || holder.dataset.visible !== "true" || holder.dataset.loading !== loadId) return;
            const viewport = page.getViewport({ scale });
            const canvas = document.createElement("canvas"); const context = canvas.getContext("2d")!;
            canvas.width = Math.ceil(viewport.width); canvas.height = Math.ceil(viewport.height);
            holder.style.width = `${viewport.width}px`; holder.style.height = `${viewport.height}px`;
            holder.replaceChildren(canvas);
            const task = page.render({ canvas, canvasContext: context, viewport }); rendering.set(number, task);
            await task.promise; if (rendering.get(number) === task) rendering.delete(number); firstContent();
          } catch (error) {
            if (holder.dataset.visible === "true" && holder.dataset.loading === loadId && !(error instanceof Error && error.name === "RenderingCancelledException")) holder.textContent = "This page could not be displayed.";
          }
        })();
      }
    }, { root: viewer, rootMargin: "300px" });
    for (let n = 1; n <= pdf.numPages; n++) {
      const holder = document.createElement("div"); holder.className = "pdf-page"; holder.dataset.page = String(n);
      holder.style.width = `${size.width}px`; holder.style.height = `${size.height}px`; pages.append(holder); holders.push(holder); observer.observe(holder);
    }
    if (anchor) {
      const holder = pages.querySelector<HTMLElement>(`[data-page="${anchor.page}"]`);
      if (holder) { viewer.scrollTop += holder.getBoundingClientRect().top - viewer.getBoundingClientRect().top + holder.clientHeight * anchor.fraction; viewer.scrollLeft = anchor.left; }
    }
    pendingAnchor = undefined;
    updatePage();
  };
  const zoom = (delta: number) => {
    const top = viewer.getBoundingClientRect().top;
    const visible = firstVisiblePage();
    const rect = visible?.getBoundingClientRect();
    const anchor = visible && rect ? { page: Number(visible.dataset.page), fraction: Math.max(0, Math.min(1, (top - rect.top) / Math.max(1, rect.height))), left: viewer.scrollLeft } : pendingAnchor;
    const nextScale = Math.max(.5, Math.min(3, scale + delta));
    if (nextScale === scale) return;
    scale = nextScale; pendingAnchor = anchor; void drawPages(anchor);
  };
  const pageInput = document.createElement("input"); pageInput.type = "number"; pageInput.min = "1"; pageInput.max = String(pdf.numPages); pageInput.value = "1";
  pageInput.className = "page-input"; pageInput.setAttribute("aria-label", "PDF page number");
  const pageCount = document.createElement("span"); pageCount.className = "status"; pageCount.textContent = `of ${pdf.numPages}`;
  let currentPage = 1;
  const updatePage = () => {
    const visible = firstVisiblePage();
    if (visible) currentPage = Number(visible.dataset.page);
    if (document.activeElement !== pageInput) pageInput.value = String(currentPage);
    previous.disabled = currentPage <= 1; next.disabled = currentPage >= pdf.numPages;
  };
  const goPage = (page: number) => {
    const holder = holders[page - 1]; if (!holder) return;
    holder.scrollIntoView({ block: "start" });
    currentPage = page; pageInput.value = String(page); updatePage();
  };
  const previous = button("Previous", () => goPage(Math.max(1, currentPage - 1)));
  const next = button("Next", () => goPage(Math.min(pdf.numPages, currentPage + 1)));
  pageInput.addEventListener("change", () => { const page = pdfPage(pageInput.value, pdf.numPages); if (page === null) pageInput.value = String(currentPage); else goPage(page); });
  viewer.addEventListener("scroll", updatePage);
  controls.append(previous, pageInput, pageCount, next);
  const search = document.createElement("input"); search.type = "search"; search.placeholder = "Find in PDF"; search.setAttribute("aria-label", "Find in PDF");
  const status = document.createElement("span"); status.className = "status"; status.setAttribute("aria-live", "polite");
  let lastPage = 0;
  const find = async () => {
    const query = search.value.trim().toLocaleLowerCase(); if (!query) return;
    status.textContent = "Searching…";
    for (let i = 1; i <= pdf.numPages; i++) {
      const n = ((lastPage + i - 1) % pdf.numPages) + 1;
      const content = await (await pdf.getPage(n)).getTextContent();
      const text = content.items.map(item => "str" in item ? item.str : "").join(" ").toLocaleLowerCase();
      if (text.includes(query)) { lastPage = n; pages.querySelector<HTMLElement>(`[data-page="${n}"]`)?.scrollIntoView({ block: "start" }); status.textContent = `Found on page ${n}`; return; }
    }
    status.textContent = "No matches";
  };
  search.addEventListener("input", () => { lastPage = 0; status.textContent = ""; });
  const findNext = () => { void find().catch(() => { status.textContent = "Search could not be completed"; }); };
  search.addEventListener("keydown", event => { if (event.key === "Enter") findNext(); });
  if (capabilities.zoom) {
    const out = button("−", () => zoom(-.25)); out.setAttribute("aria-label", "Zoom out"); out.title = "Zoom out (Ctrl+-)";
    const inside = button("+", () => zoom(.25)); inside.setAttribute("aria-label", "Zoom in"); inside.title = "Zoom in (Ctrl++)";
    controls.append(out, inside);
  }
  if (capabilities.search) controls.append(search, button("Find next", findNext), status);
  controls.hidden = !controls.childElementCount;
  const onKey = (event: KeyboardEvent) => {
    if (!(event.ctrlKey || event.metaKey) || event.altKey) return;
    if (event.key.toLowerCase() === "f" && capabilities.search) { event.preventDefault(); search.focus(); search.select(); }
    else if (capabilities.zoom && (event.key === "+" || event.key === "=" || event.key === "-")) { event.preventDefault(); zoom(event.key === "-" ? -.25 : .25); }
  };
  window.addEventListener("keydown", onKey);
  cleanup = () => { window.removeEventListener("keydown", onKey); viewer.removeEventListener("scroll", updatePage); renderGeneration++; observer?.disconnect(); for (const renderTask of rendering.values()) renderTask.cancel(); rendering.clear(); void task.destroy(); };
  await drawPages();
}
async function render(response: Response) {
  const { content: state, capabilities } = response;
  cleanup(); cleanup = () => {}; controls.replaceChildren(); controls.hidden = true; viewer.setAttribute("aria-busy", "false");
  filename.textContent = "name" in state ? state.name : "";
  switch (state.state) {
    case "empty": message("Open a document", "Choose a file to view it in a new window."); break;
    case "unsupported": message("Unsupported file format", `ShoruiPoppu cannot open “${state.name}”.`); break;
    case "error": message(`Could not open ${state.name}`, state.message); break;
    case "text": { viewer.className = "viewer document-view"; const pre = document.createElement("pre"); pre.className = "plain-text"; pre.textContent = state.text; viewer.replaceChildren(pre); firstContent(); break; }
    case "markdown": {
      viewer.className = "viewer document-view"; const article = document.createElement("article"); article.className = "document"; const stack: HTMLElement[] = [article];
      const allowed = new Set(["p", "h1", "h2", "h3", "h4", "h5", "h6", "blockquote", "pre", "ol", "ul", "li", "em", "strong", "del", "table", "thead", "tr", "td", "span"]);
      for (const token of state.tokens) {
        const parent = stack[stack.length - 1];
        if (token.kind === "start") { const el = document.createElement(allowed.has(token.text) ? token.text : "span"); parent.append(el); stack.push(el); }
        else if (token.kind === "end" && stack.length > 1) stack.pop();
        else if (token.kind === "text") parent.append(document.createTextNode(token.text));
        else if (token.kind === "break") parent.append(document.createElement("br"));
        else if (token.kind === "rule") parent.append(document.createElement("hr"));
      }
      viewer.replaceChildren(article); firstContent(); break;
    }
    case "docx": {
      viewer.className = "viewer document-view"; const article = document.createElement("article"); article.className = "document";
      const create = (block: Block): HTMLElement => {
        const tags: Record<string, string> = { paragraph: "p", h1: "h1", h2: "h2", h3: "h3", table: "table", row: "tr", cell: "td", bold: "strong", italic: "em", bold_italic: "strong", text: "span", image: "img" };
        const element = document.createElement(tags[block.kind] || "span");
        if (element instanceof HTMLImageElement && block.image?.startsWith("data:image/")) { element.src = block.image; element.alt = "Embedded document image"; }
        else { element.textContent = block.text; for (const child of block.children) element.append(create(child)); }
        if (block.kind === "bold_italic") element.style.fontStyle = "italic";
        applyTextStyle(element, block.style);
        if (block.style?.marker) { const marker = document.createElement("span"); marker.className = "doc-list-marker"; marker.textContent = block.style.marker; element.prepend(marker); }
        return element;
      };
      article.append(...state.blocks.map(create)); viewer.replaceChildren(article); firstContent(); break;
    }
    case "pptx": {
      viewer.className = "viewer slide-view";
      const surface = document.createElement("div"); surface.className = "slide"; surface.style.aspectRatio = `${state.slide.width} / ${state.slide.height}`;
      if (state.slide.background) surface.style.backgroundColor = state.slide.background;
      for (const item of state.slide.items) {
        const el = document.createElement(item.image ? "img" : "div"); el.className = "slide-item";
        if (item.fill) el.style.backgroundColor = item.fill;
        if (item.outline) el.style.border = `1px solid ${item.outline}`;
        if (item.geometry === "ellipse") el.style.borderRadius = "50%";
        if (item.geometry === "roundRect") el.style.borderRadius = "12%";
        if (item.rotation || item.flip_h || item.flip_v) el.style.transform = `rotate(${item.rotation}deg) scale(${item.flip_h ? -1 : 1}, ${item.flip_v ? -1 : 1})`;
        el.style.left = `${item.x}%`; el.style.top = `${item.y}%`; el.style.width = `${item.width}%`; el.style.height = `${item.height}%`;
        if (el instanceof HTMLImageElement && item.image?.startsWith("data:image/")) { el.src = item.image; el.alt = "Slide image"; }
        else if (item.paragraphs.length) {
          for (const paragraph of item.paragraphs) {
            const p = document.createElement("p"); p.className = "slide-paragraph";
            if (paragraph.align && ["left", "right", "center", "justify"].includes(paragraph.align)) p.style.textAlign = paragraph.align;
            if (paragraph.marker) { p.style.paddingLeft = `${paragraph.level * 1.25}em`; p.append(document.createTextNode(`${paragraph.marker} `)); }
            for (const run of paragraph.runs) { const span = document.createElement("span"); span.textContent = run.text; applyTextStyle(span, run.style, state.slide.width); p.append(span); }
            el.append(p);
          }
        } else el.textContent = item.text;
        surface.append(el);
      }
      viewer.replaceChildren(surface); firstContent();
      const counter = document.createElement("span"); counter.className = "status"; counter.setAttribute("aria-live", "polite"); counter.textContent = `Slide ${state.index + 1} of ${state.count}`;
      let busy = false;
      const go = async (index: number) => {
        if (busy || index === state.index || index < 0 || index >= state.count) return;
        busy = true; prev.disabled = true; next.disabled = true; counter.textContent = "Loading slide…";
        try { await render(await invoke<Response>("slide", { index })); }
        catch { counter.textContent = `Could not load slide ${index + 1}`; prev.disabled = state.index === 0; next.disabled = state.index + 1 >= state.count; busy = false; }
      };
      const prev = button("Previous", () => void go(state.index - 1)); prev.disabled = state.index === 0;
      const next = button("Next", () => void go(state.index + 1)); next.disabled = state.index + 1 >= state.count;
      prev.title = "Previous slide (Left arrow or Page Up)"; next.title = "Next slide (Right arrow or Page Down)";
      const onKey = (event: KeyboardEvent) => {
        const target = event.target;
        if (event.altKey || event.ctrlKey || event.metaKey || event.shiftKey || (target instanceof HTMLElement && (target.isContentEditable || ["INPUT", "TEXTAREA", "SELECT"].includes(target.tagName)))) return;
        const index = slideTarget(event.key, state.index, state.count);
        if (index !== null) { event.preventDefault(); void go(index); }
      };
      if (capabilities.slides) {
        controls.append(prev, counter, next); controls.hidden = false;
        window.addEventListener("keydown", onKey); cleanup = () => window.removeEventListener("keydown", onKey);
      } break;
    }
    case "grid": renderGrid(state); firstContent(); break;
    case "workbook": renderWorkbook(state, capabilities); break;
    case "pdf": await renderPdf(state, capabilities); break;
  }
}
const openButton = app.querySelector<HTMLButtonElement>("#open")!;
openButton.title = "Open file (Ctrl+O)";
openButton.addEventListener("click", () => { void invoke("open_file").catch(() => message("Could not open file picker", "The file picker could not be opened.")); });
window.addEventListener("keydown", event => { if ((event.ctrlKey || event.metaKey) && !event.altKey && event.key.toLowerCase() === "o") { event.preventDefault(); openButton.click(); } });
app.querySelector<HTMLButtonElement>("#close")!.addEventListener("click", () => { void getCurrentWindow().close().catch(() => message("Could not close window", "The window could not be closed.")); });
void invoke<Response>("viewer_state").then(render).catch(() => message("Could not initialize viewer", "The viewer could not be initialized."));
