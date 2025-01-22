import { Room, RoomAction, type RoomAnnounced } from "../room";

import type { MoqWatch } from "./watch";

import { attribute, Element, element } from "./component";
import { jsx } from "./jsx";

@element("moq-meet")
export class MoqMeet extends Element {
	#room: Room;
	#broadcasts: HTMLDivElement;

	@attribute
	accessor room = "";

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

		this.#room = new Room();
		const announced = this.#room.announced();
		this.#runAnnounced(announced).finally(() => announced.free());

		const shadow = this.attachShadow({ mode: "open" });
		this.#broadcasts = (<div css={{ display: "flex", gap: "8px", alignItems: "center" }} />) as HTMLDivElement;
		shadow.appendChild(style);
		shadow.appendChild(this.#broadcasts);
	}

	roomChange(value: string) {
		this.#room.url = value;
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

declare global {
	interface HTMLElementTagNameMap {
		"moq-meet": MoqMeet;
	}
}

export default MoqMeet;
