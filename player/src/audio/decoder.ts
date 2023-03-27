import * as Message from "./message";
import { InitParser } from "../mp4/init";
import { Renderer } from "./renderer"
import { MP4New, MP4Sample, MP4ArrayBuffer } from "../mp4/index"

export class Decoder {
    tracks: Map<string, InitParser>;
    renderer: Renderer;

    constructor(renderer: Renderer) {
        this.tracks = new Map();
        this.renderer = renderer;
    }

    async init(msg: Message.Init) {
        let track = this.tracks.get(msg.track);
        if (!track) {
            track = new InitParser()
            this.tracks.set(msg.track, track)
        }

        while (1) {
            const data = await msg.stream.read()
            if (!data) break

            track.init(data)
        }

        // TODO this will hang on incomplete data
        const init = await track.ready;
        const info = init.info;

        if (info.audioTracks.length != 1 || info.videoTracks.length != 0) {
            throw new Error("expected a single audio track")
        }
    }

    async decode(msg: Message.Segment) {
		let track = this.tracks.get(msg.track);
		if (!track) {
			track = new InitParser()
			this.tracks.set(msg.track, track)
		}

		// Wait for the init segment to be fully received and parsed
		const init = await track.ready;
        const info = init.info;
        const video = info.videoTracks[0]

        const decoder = new AudioDecoder({
            output: (frame: AudioFrame) => {
                this.renderer.push(frame)
            },
            error: (err: Error) => {
                console.warn(err)
            }
        });

        decoder.configure({
            codec: info.mime,
            // TODO what else?
            // optimizeForLatency: true
        })

		const input = MP4New();

        input.onSamples = (id: number, user: any, samples: MP4Sample[]) => {
            for (let sample of samples) {
                const timestamp = sample.dts / (1000 / info.timescale) // milliseconds

                decoder.decode(new EncodedAudioChunk({
                    data: sample.data,
                    duration: sample.duration,
                    timestamp: timestamp,
                }))
            }
        }

		input.onReady = (info: any) => {
			input.setExtractionOptions(info.tracks[0].id, {}, { nbSamples: 1 });
			input.start();
		}

        let offset = 0

        // MP4box requires us to reparse the init segment unfortunately
        for (let raw of track.raw) {
            offset = input.appendBuffer(raw)
        }

		/* TODO I'm not actually sure why this code doesn't work; something trips up the MP4 parser
			while (1) {
				const data = await stream.read()
				if (!data) break

				input.appendBuffer(data)
				input.flush()
			}
		*/

		// One day I'll figure it out; until then read one top-level atom at a time
		while (!await msg.stream.done()) {
			const raw = await msg.stream.peek(4)
			const size = new DataView(raw.buffer, raw.byteOffset, raw.byteLength).getUint32(0)
			const atom = await msg.stream.bytes(size)

            // Make a copy of the atom because mp4box only accepts an ArrayBuffer unfortunately
            let box = new Uint8Array(atom.byteLength);
            box.set(atom)

            // and for some reason we need to modify the underlying ArrayBuffer with offset
            let buffer = box.buffer as MP4ArrayBuffer
            buffer.fileStart = offset

            // Parse the data
            offset = input.appendBuffer(buffer)
            input.flush()
		}
    }
}