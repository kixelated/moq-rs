import * as Moq from "@kixelated/moq";

export class Broadcast {
	#connection: Moq.Connection;
	#name: string;

	constructor(connection: Moq.Connection, name: string) {
		this.#connection = connection;
		this.#name = name;
	}

	publish(track: Moq.TrackReader) {
		this.#connection.publish(this.#name, track);
	}

	close() {
		this.#connection.close();
	}
}
