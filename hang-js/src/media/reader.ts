import * as Moq from "@kixelated/moq";
import { Frame } from "./frame";

export class Reader {
	#track: Moq.TrackReader;
	#group?: Moq.GroupReader;

	#nextGroup?: Promise<Moq.GroupReader | undefined>;
	#nextFrame?: Promise<Frame | undefined>;

	constructor(track: Moq.TrackReader) {
		this.#track = track;
		this.#nextGroup = track.nextGroup();
	}

	async readFrame(): Promise<Frame | undefined> {
		for (;;) {
			if (this.#nextGroup && this.#nextFrame) {
				// Figure out if nextFrame or nextGroup wins the race.
				// TODO support variable latency
				const next: { frame: Frame | undefined } | { group: Moq.GroupReader | undefined } = await Promise.race([
					this.#nextFrame.then((frame) => ({ frame })),
					this.#nextGroup.then((group) => ({ group })),
				]);

				if ("frame" in next) {
					const frame = next.frame;
					if (!frame) {
						// The group is done, wait for the next one.
						this.#nextFrame = undefined;
						continue;
					}

					// We got a frame; return it.
					// But first start fetching the next frame (note: group cannot be undefined).
					this.#nextFrame = this.#group ? Frame.decode(this.#group, false) : undefined;
					return frame;
				}

				const group = next.group;
				if (!group) {
					// The track is done, but finish with the current group.
					this.#nextGroup = undefined;
					continue;
				}

				// Start fetching the next group.
				this.#nextGroup = this.#track.nextGroup();

				if (this.#group && this.#group.id >= group.id) {
					// Skip this old group.
					console.warn(`skipping old group: ${group.id} < ${this.#group.id}`);
					group.close();
					continue;
				}

				// Skip the rest of the current group.
				this.#group?.close();
				this.#group = next.group;
				this.#nextFrame = this.#group ? Frame.decode(this.#group, true) : undefined;
			} else if (this.#nextGroup) {
				// Wait for the next group.
				const group = await this.#nextGroup;
				this.#group?.close();
				this.#group = group;

				if (this.#group) {
					this.#nextGroup = this.#track.nextGroup();
					this.#nextFrame = Frame.decode(this.#group, true);
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

				this.#nextFrame = this.#group ? Frame.decode(this.#group, false) : undefined;
				return frame;
			} else {
				return undefined;
			}
		}
	}

	close() {
		this.#nextFrame = undefined;
		this.#nextGroup = undefined;

		this.#group?.close();
		this.#group = undefined;

		this.#track.close();
	}
}
