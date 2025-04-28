import path from "node:path";

import { fileURLToPath } from "node:url";
import HtmlWebpackPlugin from "html-webpack-plugin";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const config = {
	entry: "./src/demo/index.ts",
	output: {
		path: path.resolve(__dirname, "out"),
		filename: "index.js",
	},
	plugins: [
		new HtmlWebpackPlugin({
			template: "src/demo/watch.html",
			filename: "index.html",
		}),
		new HtmlWebpackPlugin({
			template: "src/demo/publish.html",
			filename: "publish.html",
		}),
	],
	mode: "development",
	experiments: {
		asyncWebAssembly: true,
		topLevelAwait: true,
		css: true,
	},
	// Typescript support
	module: {
		rules: [
			{
				test: /\.ts(x)?$/,
				loader: "builtin:swc-loader",
				exclude: /node_modules/,
			},
			{
				test: /\.css$/,
				use: ["postcss-loader"],
				type: "css",
			},
		],
		parser: {
			javascript: {
				worker: ["*context.audioWorklet.addModule()", "..."],
			},
		},
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
