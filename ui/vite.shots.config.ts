import path from "node:path";
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
// Scratch config for screenshots: proxies to the test instance on 8383, never 8080.
const server = "http://localhost:8383";
export default defineConfig({
  base: "/app/",
  plugins: [react(), tailwindcss()],
  resolve: { alias: { "@": path.resolve(import.meta.dirname, "src") } },
  server: { strictPort: true, proxy: Object.fromEntries(["/api", "/events", "/fonts", "/slide.js", "/slide.css", "/preview.mjpg", "/youtube"].map((p) => [p, server])) },
});
