import type { RoomAnnounced } from "@dist/rust";
import { Room, RoomAction } from "@dist/rust";
import type { MoqPublishElement } from "./publish";
import { MoqWatchElement } from "./watch";

export class MoqMeetElement extends HTMLElement {
	#room: Room;
	#publish?: MoqPublishElement;

	#broadcasts: HTMLDivElement;

	// Work-around to make sure we only have one instance of each user.
	#unique: Map<string, number> = new Map();

	static get observedAttributes() {
		return ["room"];
	}

	constructor() {
		super();

		this.#room = new Room();

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
			case "room":
				this.#room.url = value;
				break;
		}
	}

	async #runAnnounced(announced: RoomAnnounced) {
		while (true) {
			const announce = await announced.next();
			if (!announce) {
				return;
			}

			console.log(announce);

			switch (announce.action) {
				case RoomAction.Join:
					this.#join(announce.name);
					break;
				case RoomAction.Leave:
					this.#leave(announce.name);
					break;
				case RoomAction.Live:
					// TODO
					break;
			}
		}
	}

	#join(name: string) {
		const count = this.#unique.get(name);
		if (count) {
			this.#unique.set(name, count + 1);
			return;
		}

		this.#unique.set(name, 1);

		const watch = new MoqWatchElement();
		watch.id = name;
		watch.url = `${this.room}/${name}`;
		this.#broadcasts.appendChild(watch);
	}

	#leave(name: string) {
		let count = this.#unique.get(name);
		const watch = this.#broadcasts.querySelector(`#${name}`) as MoqWatchElement | null;

		if (!count || !watch) {
			throw new Error("user not found");
		}

		count -= 1;

		if (count === 0) {
			this.#unique.delete(name);
			watch.remove();
		} else {
			this.#unique.set(name, count);
		}
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
}

customElements.define("moq-meet", MoqMeetElement);

declare global {
	interface HTMLElementTagNameMap {
		"moq-meet": MoqMeetElement;
	}
}
