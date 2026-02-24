import path from "path";
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import { visualizer } from "rollup-plugin-visualizer";

// @ts-expect-error process is a nodejs global
const host = process.env.TAURI_DEV_HOST;

export default defineConfig(async () => ({
  plugins: [
    tailwindcss(),
    react(),
    visualizer({ filename: "dist/stats.html", gzipSize: true, open: false }),
  ],

  build: {
    rollupOptions: {
      output: {
        manualChunks(id: string) {
          // Shared runtime first — must be assigned before feature chunks to
          // prevent preact internals landing in the first feature chunk that
          // imports them (which creates circular dashboard ↔ markdown deps).
          if (id.includes("node_modules/preact")) return "vendor";
          if (id.includes("/components/settings/")) return "settings";
          if (id.includes("/components/dashboard/")) return "dashboard";
          if (
            id.includes("react-markdown") ||
            id.includes("remark-gfm") ||
            id.includes("unified") ||
            id.includes("mdast-util-to-hast") ||
            id.includes("remark-parse")
          )
            return "markdown";
        },
      },
    },
  },

  resolve: {
    alias: {
      "@renderer": path.resolve(__dirname, "src/renderer"),
      "@shared": path.resolve(__dirname, "src/shared"),
      "@main": path.resolve(__dirname, "src/main"),
      // Preact compat: redirect React imports to Preact at bundle time.
      // Rollback: delete these four lines and remove preact from package.json.
      "react": "preact/compat",
      "react-dom/client": "preact/compat/client",
      "react-dom": "preact/compat",
      "react/jsx-runtime": "preact/jsx-runtime",
    },
  },

  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      ignored: ["**/src-tauri/**"],
    },
  },
}));
