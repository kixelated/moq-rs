import type { RoomAnnounced } from "@dist/rust";
import { Room, RoomAction } from "@dist/rust";
import { MoqPublishElement } from "./publish";
import { MoqWatchElement } from "./watch";

export class MoqMeetElement extends HTMLElement {
	#room: Room;
	#publish: MoqPublishElement;

	#broadcasts: HTMLDivElement;

	static get observedAttributes() {
		return ["room", "name"];
	}

	constructor() {
		super();

		this.#room = new Room();
		this.#publish = new MoqPublishElement();

		const announced = this.#room.announced();
		this.#runAnnounced(announced).finally(() => announced.free());

		const shadow = this.attachShadow({ mode: "open" });
		shadow.innerHTML = `
			<style type="text/css">
				.broadcasts {
					display: flex;
					gap: 8px;
					align-items: center;

					moq-watch, moq-publish {
						border-radius: 0.375rem;
						overflow: hidden;
					}
				}

			</style>
		`;

		this.#broadcasts = document.createElement("div");
		this.#broadcasts.className = "broadcasts";
		this.#broadcasts.appendChild(this.#publish);

		shadow.appendChild(this.#broadcasts);
	}

	connectedCallback() {
		for (const name of MoqMeetElement.observedAttributes) {
			const value = this.getAttribute(name);
			if (value !== undefined) {
				this.attributeChangedCallback(name, null, value);
			}
		}
	}

	disconnectedCallback() {}

	attributeChangedCallback(name: string, old: string | null, value: string | null) {
		if (old === value) {
			return;
		}

		switch (name) {
			// biome-ignore lint/suspicious/noFallthroughSwitchClause: Update the publish URL when the room or name changes.
			case "room":
				this.#room.url = value;
			case "name":
				if (this.room && this.name) {
					this.#publish.url = `${this.room}/${this.name}`;
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
		if (name === this.name) {
			// TODO use this as a signal that we're officially in the room.
			return;
		}

		const watch = new MoqWatchElement();
		watch.id = name;
		watch.url = `${this.room}/${name}`;

		this.#broadcasts.appendChild(watch);
	}

	#leave(name: string) {
		if (name === this.name) {
			// TODO use this as a signal that we got kicked out of the room.
			// Sucks to suck.
			return;
		}

		const watch = this.#broadcasts.querySelector(`#${name}`) as MoqWatchElement | null;
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

	get name(): string | null {
		return this.getAttribute("name");
	}

	set name(value: string | null) {
		if (value === null || value === "") {
			this.removeAttribute("name");
		} else {
			this.setAttribute("name", value);
		}
	}
}

customElements.define("moq-meet", MoqMeetElement);

declare global {
	interface HTMLElementTagNameMap {
		"moq-meet": MoqMeetElement;
	}
}
