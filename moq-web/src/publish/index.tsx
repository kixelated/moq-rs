import * as Rust from "@rust";
import { type ConnectionStatus, convertConnectionStatus } from "../connection";
import { MoqElement, attribute, element, jsx } from "../util";

export type PublishDevice = "camera" | "screen" | "none" | "";

@element("moq-publish")
export class Publish extends MoqElement {
	#rust: Rust.Publish;

	// Optionally preview our media stream.
	#preview: HTMLVideoElement;

	@attribute
	accessor url = "";

	@attribute
	accessor device: PublishDevice = "";

	@attribute
	accessor preview = false;

	constructor() {
		super();

		this.#rust = new Rust.Publish();

		const style = (
			<style>
				{`
				:host {
					display: block;
					position: relative;

					max-width: 100%;
					max-height: 100%;
				}
				`}
			</style>
		);

		this.#preview = (
			<video
				css={{
					objectFit: "contain",
					maxWidth: "100%",
					maxHeight: "100%",
					display: "none",
				}}
				autoplay
			/>
		) as HTMLVideoElement;

		const shadow = this.attachShadow({ mode: "open" });
		shadow.appendChild(style);
		shadow.appendChild(this.#preview);
	}

	urlChange(url: string) {
		this.#rust.url = url !== "" && this.device !== "" ? url : null;
	}

	async deviceChange(value: PublishDevice) {
		switch (value) {
			case "camera":
				await this.shareCamera();
				break;
			case "screen":
				await this.shareScreen();
				break;
			case "none":
				this.shareNothing();
				break;
			case "":
				break;
			default: {
				const _: never = value;
				throw new Error(`Invalid media kind: ${value}`);
			}
		}

		this.urlChange(this.url); // Update the URL with the new device.
		this.#updatePreview();
	}

	previewChange(value: boolean) {
		this.#preview.style.display = value ? "" : "none";
		this.#updatePreview();
	}

	#updatePreview() {
		this.#preview.srcObject = this.preview && this.#rust.media ? this.#rust.media : null;
	}

	share(media: MediaStream | null) {
		if (media === this.#rust.media) {
			return;
		}

		for (const track of this.#rust.media?.getTracks() || []) {
			track.stop();
		}

		this.#rust.media = media;
		this.#updatePreview();
	}

	// Helpers to make sharing easier.
	async shareCamera() {
		this.share(await navigator.mediaDevices.getUserMedia({ video: true }));
	}

	async shareScreen() {
		this.share(await navigator.mediaDevices.getDisplayMedia({ video: true }));
	}

	shareNothing() {
		this.share(null);
	}

	async *connectionStatus(): AsyncGenerator<ConnectionStatus> {
		const status = await this.#rust.status();

		for (;;) {
			const next = await status.connection();
			yield convertConnectionStatus(next);
		}
	}
}

declare global {
	interface HTMLElementTagNameMap {
		"moq-publish": Publish;
	}
}
