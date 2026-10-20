import path from "node:path";
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";

// The build lands in ../web/app, which the Rust program embeds and serves at "/".
// slide.js and slide.css come from the program itself (shared with the TVs).
const server = "http://localhost:8080";
export default defineConfig({
  base: "/app/",
  plugins: [react(), tailwindcss()],
  resolve: { alias: { "@": path.resolve(import.meta.dirname, "src") } },
  build: { outDir: "../web/app", emptyOutDir: true, chunkSizeWarningLimit: 1500 },
  server: {
    proxy: Object.fromEntries(
      ["/api", "/events", "/fonts", "/slide.js", "/slide.css", "/preview.mjpg", "/youtube"].map((p) => [p, server]),
    ),
  },
});
