import adapter from "@sveltejs/adapter-static";
import { vitePreprocess } from "@sveltejs/vite-plugin-svelte";
import "dotenv/config";
import process from "node:process";
import { sveltekit } from "@sveltejs/kit/vite";
import tailwindcss from "@tailwindcss/vite";
import rollupPluginLicense from "rollup-plugin-license";
import { defineConfig } from "vite";

export default defineConfig({
	build: {
		rollupOptions: {
			plugins: [
				rollupPluginLicense({
					thirdParty: {
						multipleVersions: true,
						output: {
							file: ".svelte-kit/output/client/THIRD-PARTY.txt",
						},
					},
				}),
			],
		},
	},
	plugins: [
		tailwindcss(),
		sveltekit({
			preprocess: vitePreprocess(),
			adapter: adapter({ fallback: "404.html" }),
			csp: {
				directives: {
					"default-src": ["self"],
					"connect-src": ["self", process.env.PUBLIC_API_HOST ? `https://${process.env.PUBLIC_API_HOST}` : "", process.env.PUBLIC_API_HOST ? `wss://${process.env.PUBLIC_API_HOST}` : ""],
					"img-src": ["self", "data:"],
					"style-src": ["self", "unsafe-inline"],
				},
			},
		}),
	],
	server: {
		allowedHosts: true,
		proxy: {
			"/api": { target: "http://localhost:8000", ws: true },
		},
	},
});
