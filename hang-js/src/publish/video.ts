import * as Moq from "@kixelated/moq";
import { Frame } from "../container/frame";
import * as Catalog from "../catalog";
import { VideoTrackSettings } from "../util/settings";

const SUPPORTED = [
	"avc1", // H.264
	"hev1", // HEVC (aka h.265)
	// "av01", // TDOO support AV1
];

export class Video {
	#group?: Moq.GroupWriter;

	readonly track: Moq.Track;
	readonly media: MediaStreamVideoTrack;
	readonly catalog: Catalog.Video;
	readonly settings: VideoTrackSettings;

	#encoder: VideoEncoder;

	// true if we should insert a keyframe, undefined when the encoder should decide
	#keyframeNext: true | undefined = true;

	// Count the number of frames without a keyframe.
	#keyframeCounter = 0;

	constructor(media: MediaStreamVideoTrack) {
		this.media = media;
		this.settings = media.getSettings() as VideoTrackSettings;
		this.track = new Moq.Track("video", 2);

		this.catalog = {
			track: {
				name: "video",
				priority: 2,
			},
			codec: "vp8",
			resolution: {
				width: this.settings.width,
				height: this.settings.height,
			},
		};

		this.#encoder = new VideoEncoder({
			output: (frame, metadata) => this.#encoded(frame, metadata),
			error: (err) => this.track.writer.abort(err),
		});

		this.#encoder.configure({
			codec: this.catalog.codec,
			width: this.catalog.resolution.width,
			height: this.catalog.resolution.height,
			latencyMode: "realtime",
		});

		this.#run()
			.catch((err) => this.track.writer.abort(err))
			.finally(() => this.track.writer.close());
	}

	static async isSupported(config: VideoEncoderConfig) {
		// Check if we support a specific codec family
		const short = config.codec.substring(0, 4);
		if (!SUPPORTED.includes(short)) return false;

		// Default to hardware encoding
		config.hardwareAcceleration ??= "prefer-hardware";

		// Default to CBR
		config.bitrateMode ??= "constant";

		// Default to realtime encoding
		config.latencyMode ??= "realtime";

		const res = await VideoEncoder.isConfigSupported(config);
		return !!res.supported;
	}

	async #run() {
		const input = new MediaStreamTrackProcessor({ track: this.media });
		const reader = input.readable.getReader();

		for (;;) {
			const result = await reader.read();
			if (!result || result.done) {
				break;
			}

			const frame = result.value;

			// Set keyFrame to undefined when we're not sure so the encoder can decide.
			this.#encoder.encode(frame, { keyFrame: this.#keyframeNext });
			this.#keyframeNext = undefined;
			frame.close();
		}
	}

	async #encoded(frame: EncodedVideoChunk, metadata?: EncodedVideoChunkMetadata) {
		if (frame.type === "key") {
			this.#keyframeCounter = 0;
			this.#group?.close();
			this.#group = this.track.writer.appendGroup();
		} else {
			this.#keyframeCounter += 1;
			const framesPerGop = this.settings.frameRate ? 2 * this.settings.frameRate : 60;
			if (this.#keyframeCounter + this.#encoder.encodeQueueSize >= framesPerGop) {
				this.#keyframeNext = true;
			}
		}

		if (!this.#group) throw new Error("missing keyframe");

		const buffer = new Uint8Array(frame.byteLength);
		frame.copyTo(buffer);

		const karp = new Frame(frame.type === "key", frame.timestamp, buffer);
		karp.encode(this.#group);
	}

	close() {
		this.track.writer.close();
		this.#encoder.close();
	}
}
