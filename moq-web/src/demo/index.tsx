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

import type SlButton from "@shoelace-style/shoelace/dist/components/button/button.js";
import type SlInput from "@shoelace-style/shoelace/dist/components/input/input.js";
import type SlRadioButton from "@shoelace-style/shoelace/dist/components/radio-group/radio-group.js";

import { adjectives, animals, uniqueNamesGenerator } from "unique-names-generator";
import { attribute, element, Element } from "../element/component";
import { jsx } from "../element/jsx";

// TODO This is a tree shaking work-around.
export { MoqMeet, MoqPublish, MoqWatch };

@element("moq-demo")
export class MoqDemo extends Element {
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

	roomChange(value: string) {
		this.#meet.room = value;
	}

	mediaChange(value: string) {
		if (value !== "camera" && value !== "screen" && value !== "") {
			throw new Error(`Invalid media: ${value}`);
		}

		this.#publish.media = value ?? "";
	}

	joinChange(value: boolean) {
		if (value) {
			this.#publish.url = `${this.#meet.room}/${this.name}`;
			this.#leave.style.display = "";
		} else {
			this.#publish.url = "";
			this.#leave.style.display = "none";
		}
	}
}

declare global {
	interface HTMLElementTagNameMap {
		"moq-demo": MoqDemo;
	}
}
