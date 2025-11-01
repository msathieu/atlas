import adapter from "@sveltejs/adapter-static";
import { vitePreprocess } from "@sveltejs/vite-plugin-svelte";
import "dotenv/config";
import process from "node:process";

const config = {
  preprocess: vitePreprocess(),
  kit: {
    adapter: adapter({
      fallback: "404.html",
    }),
    csp: {
      directives: {
        "default-src": ["self"],
        "connect-src": ["self", process.env.PUBLIC_API_HOST ? `https://${process.env.PUBLIC_API_HOST}` : "", process.env.PUBLIC_API_HOST ? `wss://${process.env.PUBLIC_API_HOST}` : ""],
        "img-src": ["self", "data:"],
        "style-src": ["self", "unsafe-inline"],
      },
    },
  },
};

export default config;
