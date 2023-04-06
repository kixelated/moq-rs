import * as Message from "./message";
import * as MP4 from "../mp4"
import * as Stream from "../stream"
import * as Util from "../util"

import Renderer from "./renderer"

export default class Decoder {
    // Store the init message for each track
    tracks: Map<string, Util.Deferred<Message.Init>>;
    decoder: AudioDecoder; // TODO one per track
    sync: Message.Sync;

    constructor(config: Message.Config, renderer: Renderer) {
        this.tracks = new Map();

        this.decoder = new AudioDecoder({
            output: renderer.emit.bind(renderer),
            error: console.warn,
        });
    }

    init(msg: Message.Init) {
		let defer = this.tracks.get(msg.track);
		if (!defer) {
            defer = new Util.Deferred()
			this.tracks.set(msg.track, defer)
		}

        if (msg.info.audioTracks.length != 1 || msg.info.videoTracks.length != 0) {
            throw new Error("Expected a single audio track")
        }

        const track = msg.info.audioTracks[0]
        const audio = track.audio

        defer.resolve(msg)
    }

    async decode(msg: Message.Segment) {
		let track = this.tracks.get(msg.track);
		if (!track) {
            track = new Util.Deferred()
			this.tracks.set(msg.track, track)
		}

		// Wait for the init segment to be fully received and parsed
		const init = await track.promise;
        const audio = init.info.audioTracks[0]

        if (this.decoder.state == "unconfigured") {
            this.decoder.configure({
                codec: audio.codec,
                numberOfChannels: audio.audio.channel_count,
                sampleRate: audio.audio.sample_rate,
            })
        }

		const input = MP4.New();

        input.onSamples = (id: number, user: any, samples: MP4.Sample[]) => {
            for (let sample of samples) {
                // Convert to microseconds
                const timestamp = 1000 * 1000 * sample.dts / sample.timescale
                const duration = 1000 * 1000 * sample.duration / sample.timescale

                // This assumes that timescale == sample rate
                this.decoder.decode(new EncodedAudioChunk({
                    type: sample.is_sync ? "key" : "delta",
                    data: sample.data,
                    duration: duration,
                    timestamp: timestamp,
                }))
            }
        }

		input.onReady = (info: any) => {
			input.setExtractionOptions(info.tracks[0].id, {}, { nbSamples: 1 });
			input.start();
		}

        // MP4box requires us to reparse the init segment unfortunately
        let offset = 0;

        for (let raw of init.raw) {
            raw.fileStart = offset
            input.appendBuffer(raw)
        }

        const stream = new Stream.Reader(msg.reader, msg.buffer)

		/* TODO I'm not actually sure why this code doesn't work; something trips up the MP4 parser
			while (1) {
				const data = await stream.read()
				if (!data) break

				input.appendBuffer(data)
				input.flush()
			}
		*/

		// One day I'll figure it out; until then read one top-level atom at a time
		while (!await stream.done()) {
			const raw = await stream.peek(4)
			const size = new DataView(raw.buffer, raw.byteOffset, raw.byteLength).getUint32(0)
			const atom = await stream.bytes(size)

            // Make a copy of the atom because mp4box only accepts an ArrayBuffer unfortunately
            let box = new Uint8Array(atom.byteLength);
            box.set(atom)

            // and for some reason we need to modify the underlying ArrayBuffer with offset
            let buffer = box.buffer as MP4.ArrayBuffer
            buffer.fileStart = offset

            // Parse the data
            offset = input.appendBuffer(buffer)
            input.flush()
		}
    }
}