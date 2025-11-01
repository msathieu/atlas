import { sveltekit } from "@sveltejs/kit/vite";
import { defineConfig } from "vite";
import tailwindcss from "@tailwindcss/vite";
import rollupPluginLicense from "rollup-plugin-license";

export default defineConfig({
  build: {
    rollupOptions: {
      plugins: [rollupPluginLicense({
        thirdParty: {
          multipleVersions: true,
          output: {
            file: "static/THIRD-PARTY.txt",
          },
        },
      })],
    },
  },
  plugins: [tailwindcss(), sveltekit()],
  server: {
    allowedHosts: true,
    proxy: {
      "/api": { target: "http://localhost:8000", ws: true },
    },
  },
});
