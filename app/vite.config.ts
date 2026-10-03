import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";

export default defineConfig({
  plugins: [svelte()],
  clearScreen: false,
  // Keep the patched fallback out of Vite's immutable dependency cache.
  optimizeDeps: { exclude: ["@kixelated/libavjs-webcodecs-polyfill"] },
  server: { watch: { ignored: ["**/src/**", "**/target/**"] } },
});
