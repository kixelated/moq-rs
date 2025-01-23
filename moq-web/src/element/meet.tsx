import { Room, RoomAction, type RoomAnnounced } from "../room";

import type { MoqWatch } from "./watch";

import { Element, attribute, element } from "./component";
import { jsx } from "./jsx";

@element("moq-meet")
export class MoqMeet extends Element {
	#room: Room;
	#container: HTMLDivElement;
	#broadcasts: Set<MoqWatch> = new Set();

	@attribute
	accessor room = "";

	@attribute
	accessor controls = false;

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
		this.#container = (<div css={{ display: "flex", gap: "8px", alignItems: "center" }} />) as HTMLDivElement;
		shadow.appendChild(style);
		shadow.appendChild(this.#container);
	}

	roomChange(value: string) {
		this.#room.url = value;
	}

	controlsChange(value: boolean) {
		for (const broadcast of this.#broadcasts) {
			broadcast.controls = value;
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
				controls={this.controls}
				css={{ borderRadius: "0.5rem", overflow: "hidden" }}
			/>
		) as MoqWatch;

		this.#container.appendChild(watch);
		this.#broadcasts.add(watch);
	}

	#leave(name: string) {
		const id = `#broadcast-${name}`;

		const watch = this.#container.querySelector(id) as MoqWatch | null;
		if (!watch) {
			console.warn(`Broadcast not found: ${id}`);
			return;
		}

		watch.remove();
		this.#broadcasts.delete(watch);
	}
}

declare global {
	interface HTMLElementTagNameMap {
		"moq-meet": MoqMeet;
	}
}

export default MoqMeet;
