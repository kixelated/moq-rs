import { dirname, resolve } from "node:path"
import { fileURLToPath } from "node:url"
import tailwindcss from "@tailwindcss/vite"
import { defineConfig } from "vite"
import dts from "vite-plugin-dts"
import solidPlugin from "vite-plugin-solid"

const __dirname = dirname(fileURLToPath(import.meta.url))

export default defineConfig({
	root: resolve(__dirname, "src/demo"),
	plugins: [tailwindcss(), solidPlugin(), dts()],
	build: {
		target: "esnext",
		outDir: "dist",
		rollupOptions: {
			input: {
				watch: "index.html",
				publish: "publish.html",
				announce: "announce.html",
			},
		},
	},
}
)
