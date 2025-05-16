// tailwind.config.js
module.exports = {
	// We only use Tailwind for the demo.
	content: ["./src/demo/*.{html,js,ts,jsx,tsx}"],
	plugins: [require("@tailwindcss/typography")],
};
