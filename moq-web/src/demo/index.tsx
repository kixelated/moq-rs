import { MoqMeet } from "../element/meet";
import { MoqPublish } from "../element/publish";
import { MoqWatch } from "../element/watch";

import "@shoelace-style/shoelace/dist/themes/light.css";
import "@shoelace-style/shoelace/dist/themes/dark.css";
import "@shoelace-style/shoelace/dist/components/button/button.js";
import "@shoelace-style/shoelace/dist/components/input/input.js";
import "@shoelace-style/shoelace/dist/components/radio-group/radio-group.js";
import "@shoelace-style/shoelace/dist/components/radio-button/radio-button.js";
import "@shoelace-style/shoelace/dist/components/icon/icon.js";
import "@shoelace-style/shoelace/dist/components/tooltip/tooltip.js";

import type SlInput from "@shoelace-style/shoelace/dist/components/input/input.js";
import type SlRadioButton from "@shoelace-style/shoelace/dist/components/radio-group/radio-group.js";
import type SlButton from "@shoelace-style/shoelace/dist/components/button/button.js";

import { uniqueNamesGenerator, adjectives, animals } from "unique-names-generator";
import { jsx } from "../element/jsx";
import { attribute } from "../element/component";

// TODO This is a tree shaking work-around.
export { MoqMeet, MoqPublish, MoqWatch };

const observedAttributes = ["media", "room", "join"] as const;
type ObservedAttribute = (typeof observedAttributes)[number];

export class MoqDemo extends HTMLElement {
	#meet: MoqMeet;
	#publish: MoqPublish;
	#leave: SlButton;

	@attribute
	accessor media = "";

	@attribute
	accessor room = "";

	@attribute
	accessor join = false;

	@attribute
	accessor name = "";

	// TODO Make this automatically generated via @attribute?
	static get observedAttributes() {
		return observedAttributes;
	}

	constructor() {
		super();

		this.#publish = (<moq-publish />) as MoqPublish;

		// Use the ?name parameter or generate a random name.
		const urlParams = new URLSearchParams(window.location.search);
		this.name =
			urlParams.get("name") ||
			uniqueNamesGenerator({
				dictionaries: [adjectives, animals],
				separator: "-",
				length: 2,
			});

		const shadow = this.attachShadow({ mode: "open" });

		const nameInput = (<sl-input clearable placeholder={this.name} />) as SlInput;
		nameInput.addEventListener("sl-change", () => {
			this.name = nameInput.value;
		});

		const cameraSelect = (
			<sl-radio-button
				onclick={() => {
					this.#publish.media = "camera";
				}}
			>
				<sl-icon slot="prefix" name="camera" label="Camera" />
			</sl-radio-button>
		) as SlRadioButton;

		const screenSelect = (
			<sl-radio-button>
				<sl-icon slot="prefix" name="display" label="Screen" />
			</sl-radio-button>
		) as SlRadioButton;

		const noneSelect = (
			<sl-radio-button>
				<sl-icon slot="prefix" name="x" label="None" />
			</sl-radio-button>
		) as SlRadioButton;

		cameraSelect.addEventListener("click", () => {
			this.#publish.media = "camera";
			this.join = true;
		});
		screenSelect.addEventListener("click", () => {
			this.#publish.media = "screen";
			this.join = true;
		});
		noneSelect.addEventListener("click", () => {
			this.#publish.media = "";
			this.join = true;
		});

		this.#leave = (
			<sl-button variant="danger" css={{ display: "none" }}>
				Leave
				<sl-icon slot="suffix" name="box-arrow-right" />
			</sl-button>
		) as SlButton;

		this.#leave.addEventListener("click", () => {
			this.join = false;
		});

		// Let the caller slot the <moq-meet> element so they have access to it.
		this.#meet = new MoqMeet();

		shadow.appendChild(this.#publish);
		shadow.appendChild(
			<div css={{ display: "flex", justifyContent: "center", gap: "16px", marginBottom: "8px" }}>
				<sl-tooltip
					content="The broadcast name. Use the same name to resume a broadcast after a crash/reload."
					placement="bottom-start"
				>
					{nameInput}
				</sl-tooltip>

				<sl-radio-group id="media" value="camera">
					<sl-tooltip content="Publish your webcam." placement="bottom">
						{cameraSelect}
					</sl-tooltip>
					<sl-tooltip content="Publish a screen or window." placement="bottom">
						{screenSelect}
					</sl-tooltip>
					<sl-tooltip content="Publish nothing (for now), but still join the meeting." placement="bottom">
						{noneSelect}
					</sl-tooltip>
				</sl-radio-group>

				<sl-tooltip content="Disconnect and stop broadcasting." placement="bottom-end">
					{this.#leave}
				</sl-tooltip>
			</div>,
		);
		shadow.appendChild(this.#meet);
	}

	attributeChangedCallback(name: ObservedAttribute, old: string | null, value: string | null) {
		if (old === value) {
			return;
		}

		switch (name) {
			case "room":
				this.#meet.room = value ?? "";
				break;
			case "media":
				if (value === "camera" || value === "screen" || value === null) {
					this.#publish.media = value ?? "";
				} else {
					throw new Error(`Unsupported media: ${value}`);
				}

				break;
			case "join":
				if (value !== null) {
					this.#publish.url = `${this.#meet.room}/${this.name}`;
					this.#leave.style.display = "";
				} else {
					this.#publish.url = "";
					this.#leave.style.display = "none";
				}

				break;
			default: {
				// Exhaustiveness check ensures all attributes are handled
				const _exhaustive: never = name;
				throw new Error(`Unhandled attribute: ${_exhaustive}`);
			}
		}
	}
}

customElements.define("moq-demo", MoqDemo);

declare global {
	interface HTMLElementTagNameMap {
		"moq-demo": MoqDemo;
	}
}
