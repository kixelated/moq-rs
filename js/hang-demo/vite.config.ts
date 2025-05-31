import tailwindcss from "@tailwindcss/vite";
import { defineConfig } from "vite";
import solidPlugin from "vite-plugin-solid";

export default defineConfig({
	root: "src",
	plugins: [tailwindcss(), solidPlugin()],
	build: {
		target: "esnext",
		rollupOptions: {
			input: {
				watch: "index.html",
				publish: "publish.html",
				announce: "announce.html",
			},
		},
	},
	server: {
		// TODO: properly support HMR
		hmr: false,
	},
});
