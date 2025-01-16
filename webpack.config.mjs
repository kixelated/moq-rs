// @ts-check

import path from "node:path";

import { fileURLToPath } from "node:url";
import WasmPackPlugin from "@wasm-tool/wasm-pack-plugin";
import HtmlWebpackPlugin from "html-webpack-plugin";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const config = {
	entry: "./moq-web/src/demo/index.ts",
	output: {
		path: path.resolve(__dirname, "dist"),
		filename: "index.js",
	},
	plugins: [
		new HtmlWebpackPlugin({
			template: "moq-web/src/demo/index.html",
		}),
		new WasmPackPlugin({
			crateDirectory: path.resolve(__dirname, "moq-web"),
			outDir: path.resolve(__dirname, "dist"),
			args: "--log-level warn",
			outName: "rust",
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
				test: /\.ts(x)?$/,
				loader: "ts-loader",
				exclude: /node_modules/,
			},
		],
	},
	resolve: {
		extensions: [".ts", ".js"],
		alias: {
			"@dist": path.resolve(__dirname, "dist"),
		},
	},
	devServer: {
		open: true,
		hot: false,
		liveReload: false,
	},
};

export default config;
