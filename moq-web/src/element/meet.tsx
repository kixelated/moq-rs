import type { RoomAnnounced } from "@dist/rust";
import { Room, RoomAction } from "@dist/rust";

import type { MoqPublishElement } from "./publish";
import type { MoqWatchElement } from "./watch";

import { jsx } from "./jsx";

const observedAttributes = ["room", "publish"] as const;
type ObservedAttribute = (typeof observedAttributes)[number];

export class MoqMeetElement extends HTMLElement {
	#room: Room;
	#publish: MoqPublishElement;

	#broadcasts: HTMLDivElement;

	static get observedAttributes() {
		return observedAttributes;
	}

	constructor() {
		super();

		this.#room = new Room();
		this.#publish = (<moq-publish />) as MoqPublishElement;

		const announced = this.#room.announced();
		this.#runAnnounced(announced).finally(() => announced.free());

		const shadow = this.attachShadow({ mode: "open" });

		this.#broadcasts = (
			<div
				css={{
					display: "flex",
					gap: "8px",
					alignItems: "center",
				}}
			/>
		) as HTMLDivElement;

		shadow.appendChild(this.#broadcasts);
	}

	connectedCallback() {
		this.#broadcasts.appendChild(this.#publish);

		for (const name of MoqMeetElement.observedAttributes) {
			const value = this.getAttribute(name);
			if (value !== undefined) {
				this.attributeChangedCallback(name, null, value);
			}
		}
	}

	disconnectedCallback() {
		this.#broadcasts.remove();
	}

	attributeChangedCallback(name: ObservedAttribute, old: string | null, value: string | null) {
		if (old === value) {
			return;
		}

		switch (name) {
			// biome-ignore lint/suspicious/noFallthroughSwitchClause: Update the publish URL when the room or name changes.
			case "room":
				this.#room.url = value;
			case "publish":
				if (this.room && this.publish) {
					this.#publish.url = `${this.room}/${this.publish}`;
				} else {
					this.#publish.url = null;
				}
				break;
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

	#join(name: string) {
		if (name === this.publish) {
			// TODO use this as a signal that we're officially in the room.
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
		if (name === this.publish) {
			// TODO use this as a signal that we got kicked out of the room.
			// Sucks to suck.
			return;
		}

		const id = `#broadcast-${name}`;
		const watch = this.#broadcasts.querySelector(id) as MoqWatchElement | null;
		if (!watch) {
			throw new Error("user not found");
		}

		watch.remove();
	}

	get room(): string | null {
		return this.getAttribute("room");
	}

	set room(value: string | null) {
		if (value === null || value === "") {
			this.removeAttribute("room");
		} else {
			this.setAttribute("room", value);
		}
	}

	get publish(): string | null {
		return this.getAttribute("publish");
	}

	set publish(value: string | null) {
		if (value === null || value === "") {
			this.removeAttribute("publish");
		} else {
			this.setAttribute("publish", value);
		}
	}
}

customElements.define("moq-meet", MoqMeetElement);

declare global {
	interface HTMLElementTagNameMap {
		"moq-meet": MoqMeetElement;
	}
}
