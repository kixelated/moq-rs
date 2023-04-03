import * as Message from "./message";
import * as MP4 from "../mp4"
import * as Stream from "../stream"
import * as Util from "../util"

import Renderer from "./renderer"

export default class Decoder {
    // Store the init message for each track
    tracks: Map<string, Util.Deferred<Message.Init>>
    renderer: Renderer;

    constructor(renderer: Renderer) {
        this.tracks = new Map();
        this.renderer = renderer;
    }

    async init(msg: Message.Init) {
		let track = this.tracks.get(msg.track);
		if (!track) {
            track = new Util.Deferred()
			this.tracks.set(msg.track, track)
		}

        if (msg.info.videoTracks.length != 1 || msg.info.audioTracks.length != 0) {
            throw new Error("Expected a single video track")
        }

        track.resolve(msg)
    }

    async decode(msg: Message.Segment) {
		let track = this.tracks.get(msg.track);
		if (!track) {
            track = new Util.Deferred()
			this.tracks.set(msg.track, track)
		}

		// Wait for the init segment to be fully received and parsed
		const init = await track.promise;
        const info = init.info;
        const video = info.videoTracks[0]

        const decoder = new VideoDecoder({
            output: (frame: VideoFrame) => {
                this.renderer.emit(frame)
            },
            error: (err: Error) => {
                console.warn(err)
            }
        });

		const input = MP4.New();

        input.onSamples = (id: number, user: any, samples: MP4.Sample[]) => {
            for (let sample of samples) {
                const timestamp = 1000 * sample.dts / sample.timescale // milliseconds

                if (sample.is_sync) {
                    // Configure the decoder using the AVC box for H.264
                    const avcc = sample.description.avcC;
                    const description = new MP4.Stream(new Uint8Array(avcc.size), 0, false)
                    avcc.write(description)

                    decoder.configure({
                        codec: video.codec,
                        codedHeight: video.track_height,
                        codedWidth: video.track_width,
                        description: description.buffer?.slice(8),
                        // optimizeForLatency: true
                    })
                }

                decoder.decode(new EncodedVideoChunk({
                    data: sample.data,
                    duration: sample.duration,
                    timestamp: timestamp,
                    type: sample.is_sync ? "key" : "delta",
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