import { readdirSync, readFileSync } from "node:fs";
import { resolve } from "node:path";
import { defineConfig, type Plugin } from "vite";

function pdfAssets(): Plugin {
  const assets = new Map<string, Buffer>();
  for (const directory of ["standard_fonts", "cmaps", "wasm"]) {
    const source = resolve("node_modules/pdfjs-dist", directory);
    for (const entry of readdirSync(source, { withFileTypes: true })) {
      if (entry.isFile()) assets.set(`${directory}/${entry.name}`, readFileSync(resolve(source, entry.name)));
    }
  }
  return {
    name: "pdfjs-local-assets",
    configureServer(server) {
      server.middlewares.use("/pdfjs/", (request, response, next) => {
        const asset = assets.get((request.url ?? "").split("?")[0].replace(/^\//, ""));
        if (!asset) return next();
        const name = (request.url ?? "").split("?")[0];
        const contentType = name.endsWith(".wasm") ? "application/wasm" : name.endsWith(".js") ? "text/javascript" : "application/octet-stream";
        response.setHeader("Content-Type", contentType);
        response.end(asset);
      });
    },
    generateBundle() {
      for (const [name, source] of assets) this.emitFile({ type: "asset", fileName: `pdfjs/${name}`, source });
    },
  };
}

export default defineConfig({
  plugins: [pdfAssets()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
  },
  envPrefix: ["VITE_", "TAURI_ENV_*"],
});
