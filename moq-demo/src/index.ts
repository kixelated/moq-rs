import "@kixelated/moq/meet";

import "@shoelace-style/shoelace/dist/themes/light.css";
import "@shoelace-style/shoelace/dist/themes/dark.css";
import "@shoelace-style/shoelace/dist/components/button/button.js";
import "@shoelace-style/shoelace/dist/components/input/input.js";
import "@shoelace-style/shoelace/dist/components/radio-group/radio-group.js";
import "@shoelace-style/shoelace/dist/components/radio-button/radio-button.js";
import "@shoelace-style/shoelace/dist/components/icon/icon.js";
import "@shoelace-style/shoelace/dist/components/tooltip/tooltip.js";

import { setBasePath } from "@shoelace-style/shoelace/dist/utilities/base-path.js";
setBasePath("/shoelace");

import { uniqueNamesGenerator, type Config, adjectives, animals } from "unique-names-generator";

const config: Config = {
	dictionaries: [adjectives, animals],
	separator: "-",
	length: 2,
};

const defaultName = uniqueNamesGenerator(config);

// Use the ?name parameter if present
const urlParams = new URLSearchParams(window.location.search);
const name = urlParams.get("name") || defaultName;

function init() {
	const nameInput = document.getElementById("name") as HTMLInputElement | null;
	if (!nameInput) {
		throw new Error("Name input not found");
	}

	nameInput.value = name;
}

if (document.readyState === "loading") {
	document.addEventListener("DOMContentLoaded", init);
} else {
	init();
}
