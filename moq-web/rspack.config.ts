// Based on https://github.com/rustwasm/wasm-bindgen/blob/main/examples/hello_world/webpack.config.js

import path from "node:path";
import rspack from "@rspack/core";
import WasmPackPlugin from "@wasm-tool/wasm-pack-plugin";
import HtmlWebpackPlugin from "html-webpack-plugin";

module.exports = {
	entry: "./src/index.ts",
	output: {
		path: path.resolve(__dirname, "dist"),
		filename: "index.js",
	},
	plugins: [
		new HtmlWebpackPlugin({
			template: "src/index.html",
		}),
		new WasmPackPlugin({
			crateDirectory: __dirname,
			outDir: path.resolve(__dirname, "pkg"),
		}),
	],
	mode: "development",
	experiments: {
		asyncWebAssembly: true,
		topLevelAwait: true,
	},
	// Typescript support
	module: {
		rules: [
			{
				test: /\.ts$/,
				exclude: /node_modules/,
				loader: "builtin:swc-loader",
				options: {
					jsc: {
						parser: {
							syntax: "typescript",
						},
					},
				},
				type: "javascript/auto",
			},
		],
	},
	resolve: {
		extensions: [".ts", ".js"],
	},
};
