import * as Moq from "@kixelated/moq";
import { decodeFrame } from "./frame";

export interface Frame {
	timestamp: number;
	data: Uint8Array;
	keyframe: boolean;
}

export interface DecoderProps {
	// The track is required to decode, but can be provided later.
	track?: Moq.TrackConsumer;

	// If epoch is provided, all timestamps will be offset by this value.
	epoch?: number;
}

// TODO This class is a mess.
export class Decoder {
	epoch: number;

	#group?: Moq.GroupConsumer;

	#track: Moq.TrackConsumer | Promise<Moq.TrackConsumer | undefined>;
	#trackResolve?: (track: Moq.TrackConsumer | undefined) => void;

	#nextGroup?: Promise<Moq.GroupConsumer | undefined>;
	#nextFrame?: Promise<Frame | undefined>;

	constructor(props?: DecoderProps) {
		this.epoch = props?.epoch ?? 0;
		this.track = props?.track;
		this.#track =
			props?.track ??
			new Promise((resolve) => {
				this.#trackResolve = resolve;
			});
	}

	get track(): Moq.TrackConsumer | undefined {
		if (this.#track instanceof Promise) return undefined;
		return this.#track;
	}

	set track(track: Moq.TrackConsumer | undefined) {
		if (this.#track instanceof Moq.TrackConsumer) {
			this.#track.close();
		}

		console.debug("setting track", track);

		this.#group?.close();
		this.#group = undefined;

		this.#nextFrame = undefined;

		if (track) {
			this.#track = track;
			this.#nextGroup = track.nextGroup();
			this.#trackResolve?.(track);
		} else {
			this.#track = new Promise((resolve) => {
				this.#trackResolve = resolve;
			});
			this.#nextGroup = undefined;
		}
	}

	async readFrame(): Promise<Frame | undefined> {
		for (;;) {
			const track = await this.#track;
			if (!track) return undefined;

			if (this.#nextGroup && this.#nextFrame) {
				// Figure out if nextFrame or nextGroup wins the race.
				// TODO support variable latency
				const next: { frame: Frame | undefined } | { group: Moq.GroupConsumer | undefined } =
					await Promise.race([
						this.#nextFrame.then((frame) => ({ frame })),
						this.#nextGroup.then((group) => ({ group })),
					]);

				console.debug("next", track.name, next);

				if ("frame" in next) {
					const frame = next.frame;
					if (!frame) {
						// The group is done, wait for the next one.
						this.#nextFrame = undefined;
						continue;
					}

					// We got a frame; return it.
					// But first start fetching the next frame (note: group cannot be undefined).
					this.#nextFrame = this.#group ? this.#decode(false) : undefined;
					return frame;
				}

				const group = next.group;
				if (!group) {
					// The track is done, but finish with the current group.
					this.#nextGroup = undefined;
					continue;
				}

				// Start fetching the next group.
				this.#nextGroup = track.nextGroup();

				if (this.#group && this.#group.id >= group.id) {
					// Skip this old group.
					console.warn(`skipping old group: ${group.id} < ${this.#group.id}`);
					group.close();
					continue;
				}

				// Skip the rest of the current group.
				this.#group?.close();
				this.#group = next.group;
				this.#nextFrame = this.#group ? this.#decode(true) : undefined;
			} else if (this.#nextGroup) {
				// Wait for the next group.
				const group = await this.#nextGroup;
				this.#group?.close();
				this.#group = group;

				if (this.#group) {
					this.#nextGroup = track.nextGroup();
					this.#nextFrame = this.#decode(true);
				} else {
					this.#nextGroup = undefined;
					this.#nextFrame = undefined;
					return undefined;
				}
			} else if (this.#nextFrame) {
				// We got the next frame, or potentially the end of the track.
				const frame = await this.#nextFrame;
				if (!frame) {
					this.#group?.close();
					this.#group = undefined;

					this.#nextFrame = undefined;
					this.#nextGroup = undefined;
					return undefined;
				}

				this.#nextFrame = this.#group ? this.#decode(false) : undefined;
				return frame;
			} else {
				return undefined;
			}
		}
	}

	async #decode(keyframe: boolean): Promise<Frame | undefined> {
		if (!this.#group) throw new Error("no group");

		const buffer = await this.#group.readFrame();
		if (!buffer) return undefined;

		const frame = decodeFrame(buffer);

		return { data: frame.data, keyframe, timestamp: frame.timestamp + this.epoch };
	}

	close() {
		if (this.#track instanceof Moq.TrackConsumer) {
			this.#track.close();
		}

		this.#group?.close();
		this.#group = undefined;
		this.#trackResolve?.(undefined);

		this.#nextGroup?.then((group) => group?.close());
		this.#nextGroup = undefined;

		this.#nextFrame = undefined;
	}
}
