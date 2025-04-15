import { Bridge } from "../bridge";

export class Publish {
	#bridge: Bridge;

	#name: string | null = null;
	#device: "camera" | "screen" | null = null;
	#video = true;
	#audio = true;

	constructor(bridge: Bridge) {
		this.#bridge = bridge;
	}

	get name(): string | null {
		return this.#name;
	}

	set name(name: string | null) {
		this.#name = name;
		this.#bridge.postMessage({ Publish: { Name: name ?? null } });
	}

	get device(): "camera" | "screen" | null {
		return this.#device;
	}

	get video(): boolean {
		return this.#video;
	}

	get audio(): boolean {
		return this.#audio;
	}

	set device(device: "camera" | "screen" | null) {
		this.#device = device;
	}

	set video(video: boolean) {
		this.#video = video;
	}

	set audio(audio: boolean) {
		this.#audio = audio;
	}
}
