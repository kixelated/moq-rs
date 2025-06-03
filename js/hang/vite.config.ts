import { readFileSync } from "fs";
import path from "path";
import glob from "fast-glob";
import { defineConfig } from "vite";
import solid from "vite-plugin-solid";

const pkg = JSON.parse(readFileSync("./package.json", "utf-8"));
const externals = [...Object.keys(pkg.dependencies || {}), ...Object.keys(pkg.peerDependencies || {})];

const external = (id: string) => externals.some((pkg) => id === pkg || id.startsWith(`${pkg}/`));

const input = Object.fromEntries(
	glob.sync("src/**/*.{ts,tsx}", { ignore: ["**/*.test.*"] }).map((file) => {
		const name = file.replace(/^src\//, "").replace(/\.(ts|tsx)$/, "");
		return [name, path.resolve(__dirname, file)];
	}),
);

export default defineConfig({
	plugins: [solid()],
	build: {
		target: "esnext",
		outDir: "dist",
		lib: false /*{
			entry: path.resolve(__dirname, "src/index.ts"),
			formats: ["es"],
		},*/,
		rollupOptions: {
			input,
			output: {
				entryFileNames: "[name].js",
				chunkFileNames: "[name]-[hash].js",
				format: "es",
				preserveModules: true,
				preserveModulesRoot: "src",
			},
			external,
			preserveEntrySignatures: "strict",
		},
	},
});
