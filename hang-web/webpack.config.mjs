import path from "node:path";

import { fileURLToPath } from "node:url";
import HtmlWebpackPlugin from "html-webpack-plugin";
import WasmPackPlugin from "@wasm-tool/wasm-pack-plugin";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const config = {
	entry: "./src/demo/index.ts",
	output: {
		path: path.resolve(__dirname, "dist"),
		filename: "index.js",
	},
	plugins: [
		new WasmPackPlugin({
			crateDirectory: path.resolve(__dirname, "moq-web"),
			outDir: path.resolve(__dirname, "dist"),
			args: "--log-level warn",
			outName: "rust",
		}),
		new HtmlWebpackPlugin({
			template: "src/demo/index.html",
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
		extensions: [".ts", ".tsx", ".js"],
	},
	devServer: {
		open: true,
		hot: false,
		liveReload: false,
	},
	optimization: {
		sideEffects: true,
	},
};

export default config;
