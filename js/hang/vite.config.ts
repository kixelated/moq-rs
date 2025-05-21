import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import tailwindcss from "@tailwindcss/vite";
import { defineConfig } from "vite";
import solidPlugin from "vite-plugin-solid";
import dts from "vite-plugin-dts";

const __dirname = dirname(fileURLToPath(import.meta.url));

export default defineConfig(({ mode }) => {
	const lib = mode === "lib";
	const demo = mode === "demo";

	return {
		root: demo ? resolve(__dirname, "src/demo") : undefined,
		plugins: [tailwindcss(), solidPlugin(), dts()],
		build: {
			target: "esnext",
			lib: lib
				? {
						entry: "src/index.ts",
						name: "hang",
						formats: ["es"],
						fileName: "index",
					}
				: undefined,
			outDir: "dist",
			rollupOptions: {
				input: demo
					? {
							watch: "index.html",
							publish: "publish.html",
							announce: "announce.html",
						}
					: undefined,
			},
		},
	};
});
