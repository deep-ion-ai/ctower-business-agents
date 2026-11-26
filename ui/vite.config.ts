/// <reference types="vitest/config" />
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// Porta fixa: o Tauri carrega o devUrl em `npm run tauri dev`.
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: { port: 1420, strictPort: true },
  build: { target: "es2021", outDir: "dist" },
  test: {
    environment: "jsdom",
    globals: true,
    setupFiles: ["./src/test/setup.ts"],
    // Sem processar CSS nos testes; o texto cru (?raw) passa, para o estilos.test.ts.
    css: { include: [/\.css\?raw$/] },
  },
});
