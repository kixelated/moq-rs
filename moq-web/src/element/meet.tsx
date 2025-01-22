import { Room, RoomAction, type RoomAnnounced } from "../room";

import type { MoqWatch } from "./watch";

import { attribute } from "./component";
import { jsx } from "./jsx";

const observedAttributes = ["room"] as const;
type ObservedAttribute = (typeof observedAttributes)[number];

export class MoqMeet extends HTMLElement {
	#room: Room | null = null;

	#broadcasts: HTMLDivElement;

	@attribute
	accessor room = "";

	static get observedAttributes() {
		return observedAttributes;
	}

	constructor() {
		super();

		const style = (
			<style>
				{`
				:host {
					display: block;
					overflow: hidden;
					position: relative;
				}
				`}
			</style>
		);

		const shadow = this.attachShadow({ mode: "open" });
		this.#broadcasts = (<div css={{ display: "flex", gap: "8px", alignItems: "center" }} />) as HTMLDivElement;
		shadow.appendChild(style);
		shadow.appendChild(this.#broadcasts);
	}

	connectedCallback() {
		this.#room = new Room();

		const announced = this.#room.announced();
		this.#runAnnounced(announced).finally(() => announced.free());

		for (const name of MoqMeet.observedAttributes) {
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

	#join(name: string) {
		const watch = (
			<moq-watch
				id={`broadcast-${name}`}
				url={`${this.room}/${name}`}
				css={{ borderRadius: "0.5rem", overflow: "hidden" }}
			/>
		) as MoqWatch;

		this.#broadcasts.appendChild(watch);
	}

	#leave(name: string) {
		const id = `#broadcast-${name}`;
		const watch = this.#broadcasts.querySelector(id) as MoqWatch | null;
		if (watch) {
			watch.remove();
		}
	}
}

customElements.define("moq-meet", MoqMeet);

declare global {
	interface HTMLElementTagNameMap {
		"moq-meet": MoqMeet;
	}
}

export default MoqMeet;
