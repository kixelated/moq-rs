import * as Rust from "@rust";
import { type ConnectionStatus, convertConnectionStatus } from "../connection";

export class Publish {
	#inner: Rust.Publish;
	#media: MediaStream | null = null;

	constructor() {
		this.#inner = new Rust.Publish();
	}

	// Helpers to make sharing easier.
	async shareCamera() {
		this.media = await navigator.mediaDevices.getUserMedia({ video: true });
	}

	async shareScreen() {
		this.media = await navigator.mediaDevices.getDisplayMedia({ video: true });
	}

	unshare() {
		this.media = null;
	}

	get media(): MediaStream | null {
		return this.#inner.media ?? null;
	}

	set media(media: MediaStream | null) {
		if (media === this.#media) {
			return;
		}

		for (const track of this.#media?.getTracks() || []) {
			track.stop();
		}

		this.#inner.media = media;
	}

	get url(): string | null {
		return this.#inner.url ?? null;
	}

	set url(value: string | null) {
		this.#inner.url = value;
	}

	async *connectionStatus(): AsyncGenerator<ConnectionStatus> {
		const status = await this.#inner.status();

		for (;;) {
			const next = await status.connection();
			yield convertConnectionStatus(next);
		}
	}
}
