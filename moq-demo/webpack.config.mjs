import path from "node:path";

import { fileURLToPath } from "node:url";
import HtmlWebpackPlugin from "html-webpack-plugin";
import CopyPlugin from "copy-webpack-plugin";
import MiniCssExtractPlugin from "mini-css-extract-plugin";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const config = {
	entry: "./src/index.ts",
	output: {
		path: path.resolve(__dirname, "dist"),
		filename: "index.js",
	},
	plugins: [
		new HtmlWebpackPlugin({
			template: "src/index.html",
		}),
		new MiniCssExtractPlugin(),
		new CopyPlugin({
			patterns: [
				// Copy Shoelace assets to dist/shoelace
				{
					from: path.resolve(__dirname, "node_modules/@shoelace-style/shoelace/dist/assets"),
					to: path.resolve(__dirname, "dist/assets"),
				},
			],
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
			{
				test: /\.css$/i,
				use: [MiniCssExtractPlugin.loader, "css-loader"],
			},
		],
	},
	resolve: {
		extensions: [".ts", ".tsx", ".js"],
	},
	devServer: {
		open: true,
	},
	optimization: {
		sideEffects: true,
	},
};

export default config;
