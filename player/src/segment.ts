import { Source } from "./source"
import { Init } from "./init"
import { MP4New, MP4File, MP4Sample, MP4Stream, MP4Parser, MP4ArrayBuffer } from "./mp4"

// Manage a segment download, keeping a buffer of a single sample to potentially rewrite the duration.
export class Segment {
	source: Source; // The SourceBuffer used to decode media.
	offset: number; // The byte offset in the received file so far
	samples: MP4Sample[]; // The samples ready to be flushed to the source.
	timestamp: number; // The expected timestamp of the first sample in milliseconds
	init: Init;

	dts?: number;       // The parsed DTS of the first sample
	timescale?: number; // The parsed timescale of the segment

	input: MP4File; // MP4Box file used to parse the incoming atoms.
	output: MP4File; // MP4Box file used to write the outgoing atoms after modification.

	done: boolean; // The segment has been completed

	constructor(source: Source, init: Init, timestamp: number) {
		this.source = source
		this.offset = 0
		this.done = false
		this.timestamp = timestamp
		this.init = init

		this.input = MP4New();
		this.output = MP4New();
		this.samples = [];

		this.input.onReady = (info: any) => {
			this.input.setExtractionOptions(info.tracks[0].id, {}, { nbSamples: 1 });

			this.input.onSamples = this.onSamples.bind(this)
			this.input.start();
		}

		// We have to reparse the init segment to work with mp4box
		for (let i = 0; i < init.raw.length; i += 1) {
			this.offset = this.input.appendBuffer(init.raw[i])

			// Also populate the output with our init segment so it knows about tracks
			this.output.appendBuffer(init.raw[i])
		}

		this.input.flush()
		this.output.flush()
	}

	push(data: Uint8Array) {
		if (this.done) return; // ignore new data after marked done

		// Make a copy of the atom because mp4box only accepts an ArrayBuffer unfortunately
		let box = new Uint8Array(data.byteLength);
		box.set(data)

		// and for some reason we need to modify the underlying ArrayBuffer with offset
		let buffer = box.buffer as MP4ArrayBuffer
		buffer.fileStart = this.offset

		// Parse the data
		this.offset = this.input.appendBuffer(buffer)
		this.input.flush()
	}

	onSamples(id: number, user: any, samples: MP4Sample[]) {
		if (!samples.length) return;

		if (this.dts === undefined) {
			this.dts = samples[0].dts;
			this.timescale = samples[0].timescale;
		}

		// Add the samples to a queue
		this.samples.push(...samples)
	}

	// Flushes any pending samples, returning true if the stream has finished.
	flush(): boolean {
		let stream = new MP4Stream(new ArrayBuffer(0), 0, false); // big-endian

		while (this.samples.length) {
			// Keep a single sample if we're not done yet
			if (!this.done && this.samples.length < 2) break;

			const sample = this.samples.shift()
			if (!sample) break;

			let moof = this.output.createSingleSampleMoof(sample);
			moof.write(stream);

			// adjusting the data_offset now that the moof size is known
			moof.trafs[0].truns[0].data_offset = moof.size+8; //8 is mdat header
			stream.adjustUint32(moof.trafs[0].truns[0].data_offset_position, moof.trafs[0].truns[0].data_offset);

			// @ts-ignore
			var mdat = new MP4Parser.mdatBox();
			mdat.data = sample.data;
			mdat.write(stream);
		}

		this.source.initialize(this.init)
		this.source.append(stream.buffer as ArrayBuffer)

		return this.done
	}

	// The segment has completed
	finish() {
		this.done = true
		this.flush()

		// Trim the buffer to 30s long after each segment.
		this.source.trim(30)
	}

	// Extend the last sample so it reaches the provided timestamp
	skipTo(pts: number) {
		if (this.samples.length == 0) return
		let last = this.samples[this.samples.length-1]

		const skip = pts - (last.dts + last.duration);

		if (skip == 0) return;
		if (skip < 0) throw "can't skip backwards"

		last.duration += skip

		if (this.timescale) {
			console.warn("skipping video", skip / this.timescale)
		}
	}

	buffered() {
		// Ignore if we have a single sample
		if (this.samples.length <= 1) return undefined;
		if (!this.timescale) return undefined;

		const first = this.samples[0];
		const last = this.samples[this.samples.length-1]


		return {
			length: 1,
			start: first.dts / this.timescale,
			end: (last.dts + last.duration) / this.timescale,
		}
	}
}
