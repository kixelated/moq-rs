// ChatGPT made a script that rewrites package.json file to use the correct paths.
// It's not pretty but nothing in NPM is.

import { execSync } from "node:child_process"
import { readFileSync, rmSync, writeFileSync, renameSync } from "node:fs"

console.log("🧹 Cleaning dist/...")
rmSync("dist", { recursive: true, force: true })

console.log("🛠️  Building...")
execSync("pnpm i && pnpm build", { stdio: "inherit" })

console.log("✍️  Rewriting package.json...")
const pkg = JSON.parse(readFileSync("package.json", "utf8"))

function rewritePath(p: string): string {
	return p.replace(/^\.\/src/, "./dist").replace(/\.ts(x)?$/, ".js")
}

pkg.main &&= rewritePath(pkg.main)
pkg.types &&= rewritePath(pkg.types)

if (pkg.exports) {
	for (const key in pkg.exports) {
		const val = pkg.exports[key]
		if (typeof val === "string") {
			pkg.exports[key] = rewritePath(val)
		} else if (typeof val === "object") {
			for (const sub in val) {
				if (typeof val[sub] === "string") {
					val[sub] = rewritePath(val[sub])
				}
			}
		}
	}
}

if (pkg.sideEffects) {
	pkg.sideEffects = pkg.sideEffects.map(rewritePath)
}

// biome-ignore lint/performance/noDelete: <explanation>
delete pkg.devDependencies
// biome-ignore lint/performance/noDelete: <explanation>
delete pkg.scripts

// Temporarily swap out the package.json with the one that has the correct paths.
renameSync("package.json", "package.backup.json")
try {
	writeFileSync("package.json", JSON.stringify(pkg, null, 2))

	console.log("🚀 Publishing...")
	execSync("pnpm publish --access=public --dry-run --no-git-checks", {
		stdio: "inherit",
	})
} finally {
	renameSync("package.backup.json", "package.json")
}