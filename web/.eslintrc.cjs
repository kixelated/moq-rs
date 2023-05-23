/* eslint-env node */
module.exports = {
	extends: ["eslint:recommended", "plugin:@typescript-eslint/recommended", "prettier"],
	parser: "@typescript-eslint/parser",
	plugins: ["@typescript-eslint", "prettier"],
	root: true,
	ignorePatterns: ["dist", "node_modules"],
	rules: {
		"@typescript-eslint/ban-ts-comment": "off",
		"@typescript-eslint/no-non-null-assertion": "off",
		"@typescript-eslint/no-explicit-any": "off",
		"no-unused-vars": "off", // note you must disable the base rule as it can report incorrect errors
		"@typescript-eslint/no-unused-vars": [
			"warn", // or "error"
			{
				argsIgnorePattern: "^_",
				varsIgnorePattern: "^_",
				caughtErrorsIgnorePattern: "^_",
			},
		],
		"prettier/prettier": 2, // Means error
	},
}
