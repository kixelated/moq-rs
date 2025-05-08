import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { defineConfig } from "vite";

const __dirname = dirname(fileURLToPath(import.meta.url));

export default defineConfig({
	root: "src/demo",
	build: {
		target: "esnext",
		rollupOptions: {
			input: {
				watch: resolve(__dirname, "src/demo/watch.html"),
				publish: resolve(__dirname, "src/demo/publish.html"),
			},
		},
	},
	server: {
		hmr: false,
	},
});
