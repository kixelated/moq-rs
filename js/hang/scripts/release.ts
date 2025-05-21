// ChatGPT made a script that rewrites package.json file to use the correct paths.
// It's not pretty but nothing in NPM is.

import { execSync } from "node:child_process";
import { copyFileSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";

console.log("🧹 Cleaning dist/...");
rmSync("dist", { recursive: true, force: true });

console.log("🛠️  Building...");
execSync("pnpm build", { stdio: "inherit" });

console.log("🔍 Type-checking...");
execSync("pnpm check", { stdio: "inherit" });

console.log("✍️  Rewriting package.json...");
const pkg = JSON.parse(readFileSync("package.json", "utf8"));

function rewritePath(p: string): string {
	return p.replace(/^\.\/src/, ".").replace(/\.ts(x)?$/, ".js");
}

pkg.main &&= rewritePath(pkg.main);
pkg.types &&= rewritePath(pkg.types);

if (pkg.exports) {
	for (const key in pkg.exports) {
		const val = pkg.exports[key];
		if (typeof val === "string") {
			pkg.exports[key] = rewritePath(val);
		} else if (typeof val === "object") {
			for (const sub in val) {
				if (typeof val[sub] === "string") {
					val[sub] = rewritePath(val[sub]);
				}
			}
		}
	}
}

if (pkg.sideEffects) {
	pkg.sideEffects = pkg.sideEffects.map(rewritePath);
}

if (pkg.files) {
	pkg.files = pkg.files.map(rewritePath);
}

// biome-ignore lint/performance/noDelete: <explanation>
delete pkg.devDependencies;
// biome-ignore lint/performance/noDelete: <explanation>
delete pkg.scripts;

console.log(pkg);

mkdirSync("dist", { recursive: true });
writeFileSync("dist/package.json", JSON.stringify(pkg, null, 2));

// Copy static files
console.log("📄 Copying README.md...");
copyFileSync("README.md", join("dist", "README.md"));

console.log("🚀 Publishing...");
execSync("pnpm publish --access=public", {
	stdio: "inherit",
	cwd: "dist",
});
