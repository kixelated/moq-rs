import * as Publish from "@kixelated/hang/publish";
import * as Moq from "@kixelated/moq";

import { jsx } from "./jsx";

export class Me extends HTMLElement {
	connection: Moq.ConnectionReload;
	name: string;

	#camera: Publish.Broadcast;
	#cameraVolume: Volume;

	#screen?: Publish.Broadcast;
	#screenVolume: Volume;

	constructor(connection: Moq.ConnectionReload, name: string) {
		super();

		this.connection = connection;
		this.name = name;

		const control: Partial<CSSStyleDeclaration> = {
			color: "white",
			padding: "8px 12px",
			borderRadius: "4px",
			background: "transparent",
			border: "1px solid transparent",
			backdropFilter: "blur(2px)",
			textShadow:
				"-1px -1px 0 black, 1px -1px 0 black, -1px 1px 0 black, 1px 1px 0 black",
		};

		const button: Partial<CSSStyleDeclaration> = {
			cursor: "pointer",
			...control,
		};

		this.#cameraVolume = new Volume();
		this.#screenVolume = new Volume();

		this.#camera = new Publish.Broadcast(connection, name, {
			device: "camera",
			onMedia: (media) => {
				microphoneButton.style.borderColor = media?.getAudioTracks().length
					? "white"
					: "transparent";
				cameraButton.style.borderColor = media?.getVideoTracks().length
					? "white"
					: "transparent";
				this.#cameraVolume.onMedia(media);
			},
		});

		const microphoneButton = (
			<button type="button" css={{ ...button, position: "relative" }}>
				{this.#cameraVolume.dom}🎤
			</button>
		);

		microphoneButton.addEventListener("click", () => {
			this.#camera.audio = !this.#camera.audio;
		});

		const cameraButton = (
			<button type="button" css={button}>
				📷
			</button>
		);
		cameraButton.addEventListener("click", () => {
			this.#camera.video = !this.#camera.video;
		});

		const screenButton = (
			<button type="button" css={button}>
				{this.#screenVolume.dom}🖥️
			</button>
		);
		screenButton.addEventListener("click", () => {
			if (this.#screen) {
				this.#screen.close();
				this.#screen = undefined;
				this.#screenVolume.onMedia(undefined);
				screenButton.style.borderColor = "transparent";
			} else {
				this.#screen = new Publish.Broadcast(
					this.connection,
					`${this.name}/screen`,
					{
						audio: true,
						video: true,
						device: "screen",
						onMedia: (media) => {
							this.#screenVolume.onMedia(media);
							screenButton.style.borderColor = media ? "white" : "transparent";
						},
					},
				);
			}
		});

		const chat = (
			<input type="text" placeholder="Type a message..." css={control} />
		);
		const settings = (
			<button type="button" css={button}>
				⚙️
			</button>
		);
		const fullscreen = (
			<button type="button" css={button}>
				⛶
			</button>
		);

		const root = this.attachShadow({ mode: "open" });
		root.appendChild(
			<div
				css={{
					position: "fixed",
					bottom: "0",
					left: "0",
					right: "0",
					display: "flex",
					alignItems: "center",
					padding: "8px 16px",
					gap: "8px",
				}}
			>
				{microphoneButton}
				{cameraButton}
				{screenButton}
				{chat}
				<div css={{ flexGrow: "1" }} />
				{settings}
				{fullscreen}
			</div>,
		);
	}
}

customElements.define("hang-me", Me);

// Renders a volume meter.
class Volume {
	dom: HTMLDivElement;

	#analyzer?: AnalyserNode;
	#animation?: number;
	#context?: AudioContext;
	#source?: MediaStreamAudioSourceNode;
	#data?: Uint8Array<ArrayBuffer>;

	constructor() {
		this.dom = (
			<div
				css={{
					position: "absolute",
					bottom: "0",
					left: "0",
					width: "100%",
					top: "100%",
					backgroundColor: "transparent",
				}}
			/>
		) as HTMLDivElement;
	}

	onMedia(media?: MediaStream) {
		this.dom.style.backgroundColor = "transparent";
		this.dom.style.top = "100%";

		if (this.#animation) {
			cancelAnimationFrame(this.#animation);
			this.#animation = undefined;
		}

		this.#source?.disconnect();
		this.#context?.close();
		this.#analyzer?.disconnect();

		this.#analyzer = undefined;
		this.#context = undefined;
		this.#source = undefined;

		const audio = media?.getAudioTracks().at(0);
		if (!media || !audio) {
			return;
		}

		this.#context = new AudioContext({
			sampleRate: audio.getSettings().sampleRate,
		});
		this.#analyzer = new AnalyserNode(this.#context, {
			// Monitor the last x samples of audio.
			// ex. at 48kHz, 4096 samples is 85ms.
			fftSize: 4096,
		});
		this.#data = new Uint8Array(this.#analyzer.frequencyBinCount);

		this.#source = this.#context.createMediaStreamSource(media);
		this.#source.connect(this.#analyzer);

		this.#animation = requestAnimationFrame(this.#render.bind(this));
	}

	#render() {
		if (!this.#data || !this.#analyzer) {
			return;
		}

		this.#analyzer.getByteTimeDomainData(this.#data);

		// Convert from [0, 255] to [-1, 1]
		let sum = 0;
		for (let i = 0; i < this.#data.length; i++) {
			const sample = (this.#data[i] - 128) / 128;
			sum += sample * sample;
		}
		const power = 2 * Math.sqrt(sum / this.#data.length);
		const hue = 2 ** power * 100 + 135;

		this.dom.style.backgroundColor = `hsla(${hue}, 80%, 40%, 0.75)`;
		this.dom.style.top = `${Math.min(100, 100 - power * 100)}%`;
		this.#animation = requestAnimationFrame(this.#render.bind(this));
	}
}
