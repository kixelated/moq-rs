import { Connection } from "@kixelated/hang/connection";
import * as Publish from "@kixelated/hang/publish";

import { Signal, Signals, batch, signal } from "@kixelated/hang/signals";
import { jsx } from "./jsx";

export type MeProps = {
	connection: Connection;
	room?: string;
	name?: string;
};

export class Me extends HTMLElement {
	connection: Connection;
	room: Signal<string | undefined>;
	name: Signal<string | undefined>;

	#camera: Publish.Broadcast;
	#cameraVolume: Volume;

	#screen: Publish.Broadcast;
	#screenVolume: Volume;

	#signals = new Signals();

	constructor(props: MeProps) {
		super();

		this.connection = props.connection;
		this.room = signal(props.room);
		this.name = signal(props.name);

		const path = this.#signals.derived(() => {
			const room = this.room.get();
			const name = this.name.get();
			if (!room || !name) return undefined;
			return `${room}/${name}`;
		});

		const control: Partial<CSSStyleDeclaration> = {
			color: "white",
			padding: "8px 12px",
			borderRadius: "4px",
			background: "transparent",
			border: "1px solid transparent",
			backdropFilter: "blur(2px)",
			textShadow: "-1px -1px 0 black, 1px -1px 0 black, -1px 1px 0 black, 1px 1px 0 black",
		};

		const button: Partial<CSSStyleDeclaration> = {
			cursor: "pointer",
			...control,
		};

		this.#camera = new Publish.Broadcast({
			connection: this.connection,
			device: "camera",
			video: false,
			audio: false,
		});
		this.#cameraVolume = new Volume(this.#camera.audio);
		const cameraButton = (
			<button type="button" css={button}>
				📷
			</button>
		);

		cameraButton.addEventListener("click", () => {
			const video: Publish.VideoTrackConstraints | undefined = this.#camera.video.constraints.peek()
				? undefined
				: {
						// 480p but square, so the browser can choose the best aspect ratio.
						width: { ideal: 640 },
						height: { ideal: 640 },
						frameRate: { ideal: 60 },
						facingMode: { ideal: "user" },
						resizeMode: "none",
					};
			this.#camera.video.constraints.set(video);
		});

		this.#signals.effect(() => {
			this.#camera.path.set(path.get());
		});

		const microphoneButton = (
			<button type="button" css={{ ...button, position: "relative" }}>
				{this.#cameraVolume.dom}🎤
			</button>
		);

		this.#signals.effect(() => {
			const video = this.#camera.video.media.get();
			const audio = this.#camera.audio.media.get();

			microphoneButton.style.borderColor = audio ? "white" : "transparent";
			cameraButton.style.borderColor = video ? "white" : "transparent";
		});

		microphoneButton.addEventListener("click", () => {
			const audio = this.#camera.audio.constraints.peek()
				? undefined
				: {
						channelCount: { ideal: 2, max: 2 },
						echoCancellation: { ideal: true },
						autoGainControl: { ideal: true },
						noiseCancellation: { ideal: true },
					};
			this.#camera.audio.constraints.set(audio);
		});

		this.#screen = new Publish.Broadcast({ connection: this.connection, device: "screen", publish: false });
		this.#screenVolume = new Volume(this.#screen.audio);
		this.#signals.effect(() => {
			let screenPath = path.get();
			if (screenPath) {
				screenPath = `${screenPath}/screen`;
			}
			this.#screen.path.set(screenPath);
		});

		const screenButton = (
			<button type="button" css={button}>
				{this.#screenVolume.dom}🖥️
			</button>
		);

		screenButton.addEventListener("click", () => {
			// We need to batch otherwise we'll request the device twice.
			batch(() => {
				this.#screen.video.constraints.set(
					this.#screen.video.constraints.peek()
						? undefined
						: {
								frameRate: { ideal: 60 },
								resizeMode: "none",
							},
				);
				this.#screen.audio.constraints.set(
					this.#screen.audio.constraints.peek()
						? undefined
						: {
								channelCount: { ideal: 2, max: 2 },
							},
				);
			});

			screenButton.style.borderColor = this.#screen.video.constraints.peek() ? "white" : "transparent";
		});

		this.#signals.effect(() => {
			// Publish only once we have at least one active track.
			this.#screen.publish.set(!!this.#screen.video.media.get() || !!this.#screen.audio.media.get());
		});

		const chat = <input type="text" placeholder="Type a message..." css={control} />;
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
	audio: Publish.Audio;
	dom: HTMLDivElement;

	#animation?: number;
	#signals = new Signals();

	constructor(audio: Publish.Audio) {
		this.audio = audio;
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

		this.#signals.effect(() => this.#onMedia());
	}

	#onMedia() {
		const audio = this.audio.media.get();

		this.dom.style.backgroundColor = "transparent";
		this.dom.style.top = "100%";

		if (!audio) {
			return;
		}

		const context = new AudioContext({
			sampleRate: audio.getSettings().sampleRate,
		});
		const analyzer = new AnalyserNode(context, {
			// Monitor the last x samples of audio.
			// ex. at 48kHz, 4096 samples is 85ms.
			fftSize: 4096,
		});

		const source = context.createMediaStreamSource(new MediaStream([audio]));
		source.connect(analyzer);

		// Create a buffer that we will reuse
		const data = new Uint8Array(analyzer.frequencyBinCount);
		this.#animation = requestAnimationFrame(this.#render.bind(this, analyzer, data));

		return () => {
			if (this.#animation) {
				cancelAnimationFrame(this.#animation);
				this.#animation = undefined;
			}

			analyzer.disconnect();
			source.disconnect();
			context.close();
		};
	}

	#render(analyzer: AnalyserNode, buffer: Uint8Array<ArrayBuffer>) {
		analyzer.getByteTimeDomainData(buffer);

		// Convert from [0, 255] to [-1, 1]
		let sum = 0;
		for (let i = 0; i < buffer.length; i++) {
			const sample = (buffer[i] - 128) / 128;
			sum += sample * sample;
		}
		const power = 2 * Math.sqrt(sum / buffer.length);
		const hue = 2 ** power * 100 + 135;

		this.dom.style.backgroundColor = `hsla(${hue}, 80%, 40%, 0.75)`;
		this.dom.style.top = `${Math.min(100, 100 - power * 100)}%`;
		this.#animation = requestAnimationFrame(this.#render.bind(this, analyzer, buffer));
	}
}
