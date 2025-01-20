import type { RoomAnnounced } from "@dist/rust";
import { Room, RoomAction } from "@dist/rust";

import { MoqPublishElement } from "./publish";
import type { MoqWatchElement } from "./watch";

import { jsx } from "./jsx";
import { attribute } from "./component";

const observedAttributes = ["room"] as const;
type ObservedAttribute = (typeof observedAttributes)[number];

export class MoqMeetElement extends HTMLElement {
	#room: Room | null = null;

	#publish?: MoqPublishElement;
	#broadcasts: HTMLDivElement;

	@attribute
	accessor room = "";

	static get observedAttributes() {
		return observedAttributes;
	}

	constructor() {
		super();

		const shadow = this.attachShadow({ mode: "open" });

		this.#publish = (<moq-publish />) as MoqPublishElement;

		const publishSlot = (<slot />) as HTMLSlotElement;
		publishSlot.addEventListener("slotchange", () => {
			this.#publish = publishSlot.assignedNodes().find((node) => node instanceof MoqPublishElement);
		});

		this.#broadcasts = (<div css={{ display: "flex", gap: "8px", alignItems: "center" }} />) as HTMLDivElement;

		shadow.appendChild(this.#broadcasts);
		shadow.appendChild(publishSlot);
	}

	connectedCallback() {
		this.#room = new Room();

		const announced = this.#room.announced();
		this.#runAnnounced(announced).finally(() => announced.free());

		for (const name of MoqMeetElement.observedAttributes) {
			const value = this.getAttribute(name);
			if (value !== undefined) {
				this.attributeChangedCallback(name, null, value);
			}
		}
	}

	disconnectedCallback() {
		this.#room?.free();
		this.#room = null;
	}

	attributeChangedCallback(name: ObservedAttribute, old: string | null, value: string | null) {
		if (!this.#room) {
			return;
		}

		if (old === value) {
			return;
		}

		switch (name) {
			case "room":
				this.#room.url = value;
				break;
			default: {
				// Exhaustiveness check ensures all attributes are handled
				const _exhaustive: never = name;
				throw new Error(`Unhandled attribute: ${_exhaustive}`);
			}
		}
	}

	async #runAnnounced(announced: RoomAnnounced) {
		while (true) {
			const announce = await announced.next();
			if (!announce) {
				return;
			}

			switch (announce.action) {
				case RoomAction.Join:
					this.#join(announce.name);
					break;
				case RoomAction.Leave:
					this.#leave(announce.name);
					break;
				case RoomAction.Live:
					// TODO show a message if there are no users
					break;
			}
		}
	}

	#updatePublish() {}

	#join(name: string) {
		if (this.#publish?.url.endsWith(`/${name}`)) {
			this.#broadcasts.appendChild(this.#publish);
			return;
		}

		const watch = (
			<moq-watch
				id={`broadcast-${name}`}
				url={`${this.room}/${name}`}
				css={{ borderRadius: "0.5rem", overflow: "hidden" }}
			/>
		) as MoqWatchElement;

		this.#broadcasts.appendChild(watch);
	}

	#leave(name: string) {
		if (this.#publish?.url.endsWith(`/${name}`)) {
			// We got kicked out of the room.
			this.#broadcasts.removeChild(this.#publish);
			return;
		}

		const id = `#broadcast-${name}`;
		const watch = this.#broadcasts.querySelector(id) as MoqWatchElement | null;
		if (watch) {
			watch.remove();
		}
	}
}

customElements.define("moq-meet", MoqMeetElement);

declare global {
	interface HTMLElementTagNameMap {
		"moq-meet": MoqMeetElement;
	}
}
