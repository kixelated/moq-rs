import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import tailwindcss from "@tailwindcss/vite";
import { defineConfig } from "vite";

const __dirname = dirname(fileURLToPath(import.meta.url));

export default defineConfig({
	root: "src/demo",
	plugins: [tailwindcss()],
	build: {
		target: "esnext",
		rollupOptions: {
			input: {
				watch: resolve(__dirname, "src/demo/index.html"),
				publish: resolve(__dirname, "src/demo/publish.html"),
			},
		},
	},
});
