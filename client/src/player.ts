import { Source } from "./source"
import { StreamReader, StreamWriter } from "./stream"
import { InitParser } from "./init"
import { Segment } from "./segment"
import { Track } from "./track"
import { Message, MessageInit, MessageSegment } from "./message"

///<reference path="./types/webtransport.d.ts"/>

export class Player {
	mediaSource: MediaSource;

	init: Map<number, InitParser>;
	audio: Track;
	video: Track;

	quic: Promise<WebTransport>;
	api: Promise<WritableStream>;

	// References to elements in the DOM
	vidRef: HTMLVideoElement; // The video element itself
	statsRef: HTMLElement; // The stats div
	throttleRef: HTMLButtonElement; // The throttle button
	throttleCount: number; // number of times we've clicked the button in a row

	interval: number;

	timeRef?: DOMHighResTimeStamp;

	constructor(props: any) {
		this.vidRef = props.vid
		this.statsRef = props.stats
		this.throttleRef = props.throttle
		this.throttleCount = 0

		this.mediaSource = new MediaSource()
		this.vidRef.src = URL.createObjectURL(this.mediaSource)

		this.init = new Map()
		this.audio = new Track(new Source(this.mediaSource));
		this.video = new Track(new Source(this.mediaSource));

		this.interval = setInterval(this.tick.bind(this), 100)
		this.vidRef.addEventListener("waiting", this.tick.bind(this))

		const quic = new WebTransport(props.url)
		this.quic = quic.ready.then(() => { return quic });

		// Create a unidirectional stream for all of our messages
		this.api = this.quic.then((q) => {
			return q.createUnidirectionalStream()
		})

		// async functions
		this.receiveStreams()

		// Limit to 4Mb/s
		this.sendThrottle()
	}

	async close() {
		clearInterval(this.interval);
		(await this.quic).close()
	}

	async sendMessage(msg: any) {
		const payload = JSON.stringify(msg)
		const size = payload.length + 8

		const stream = await this.api

		const writer = new StreamWriter(stream)
		await writer.uint32(size)
		await writer.string("warp")
		await writer.string(payload)
		writer.release()
	}

	throttle() {
		// Throttle is incremented each time we click the throttle button
		this.throttleCount += 1
		this.sendThrottle()

		// After 5 seconds disable the throttling
		setTimeout(() => {
			this.throttleCount -= 1
			this.sendThrottle()
		}, 5000)
	}

	sendThrottle() {
		// TODO detect the incoming bitrate instead of hard-coding
		const bitrate = 4 * 1024 * 1024 // 4Mb/s

		// Right shift by throttle to divide by 2,4,8,16,etc each time
		// Right shift by 3 more to divide by 8 to convert bits to bytes
		// Right shift by another 2 to divide by 4 to get the number of bytes in a quarter of a second
		let rate = bitrate >> (this.throttleCount + 3)
		let buffer = bitrate >> (this.throttleCount + 5) // 250ms before dropping

		const str = formatBits(8*rate) + "/s"
		this.throttleRef.textContent = `Throttle: ${ str }`;

		// NOTE: We don't use random packet loss because it's not a good simulator of how congestion works.
		// Delay-based congestion control like BBR most ignores packet loss, rightfully so.

		// Send the server a message to fake network congestion.
		// This is done on the server side at the socket-level for maximum accuracy (impacts all packets).
		this.sendMessage({
			"x-throttle": {
				rate: rate,
				buffer: buffer,
			},
		})
	}

	tick() {
		// Try skipping ahead if there's no data in the current buffer.
		this.trySeek()

		// Try skipping video if it would fix any desync.
		this.trySkip()

		// Update the stats at the end
		this.updateStats()
	}

	goLive() {
		const ranges = this.vidRef.buffered
		if (!ranges.length) {
			return
		}

		this.vidRef.currentTime = ranges.end(ranges.length-1);
		this.vidRef.play();
	}

	// Try seeking ahead to the next buffered range if there's a gap
	trySeek() {
		if (this.vidRef.readyState > 2) { // HAVE_CURRENT_DATA
			// No need to seek
			return
		}

		const ranges = this.vidRef.buffered
		if (!ranges.length) {
			// Video has not started yet
			return
		}

		for (let i = 0; i < ranges.length; i += 1) {
			const pos = ranges.start(i)

			if (this.vidRef.currentTime >= pos) {
				// This would involve seeking backwards
				continue
			}

			console.warn("seeking forward", pos - this.vidRef.currentTime)

			this.vidRef.currentTime = pos
			return
		}
	}

	// Try dropping video frames if there is future data available.
	trySkip() {
		let playhead: number | undefined

		if (this.vidRef.readyState > 2) {
			// If we're not buffering, only skip video if it's before the current playhead
			playhead = this.vidRef.currentTime
		}

		this.video.advance(playhead)
	}

	async receiveStreams() {
		const q = await this.quic
		const streams = q.incomingUnidirectionalStreams.getReader()

		while (true) {
			const result = await streams.read()
			if (result.done) break

			const stream = result.value
			this.handleStream(stream) // don't await
		}
	}

	async handleStream(stream: ReadableStream) {
		let r = new StreamReader(stream.getReader())

		while (!await r.done()) {
			const size = await r.uint32();
			const typ = new TextDecoder('utf-8').decode(await r.bytes(4));

			if (typ != "warp") throw "expected warp atom"
			if (size < 8) throw "atom too small"

			const payload = new TextDecoder('utf-8').decode(await r.bytes(size - 8));
			const msg = JSON.parse(payload) as Message

			if (msg.init) {
				return this.handleInit(r, msg.init)
			} else if (msg.segment) {
				return this.handleSegment(r, msg.segment)
			}
		}
	}

	async handleInit(stream: StreamReader, msg: MessageInit) {
		let init = this.init.get(msg.id);
		if (!init) {
			init = new InitParser()
			this.init.set(msg.id, init)
		}

		while (1) {
			const data = await stream.read()
			if (!data) break

			init.push(data)
		}
	}

	async handleSegment(stream: StreamReader, msg: MessageSegment) {
		let pending = this.init.get(msg.init);
		if (!pending) {
			pending = new InitParser()
			this.init.set(msg.init, pending)
		}

		// Wait for the init segment to be fully received and parsed
		const init = await pending.ready;

		let track: Track;
		if (init.info.videoTracks.length) {
			track = this.video
		} else {
			track = this.audio
		}

		const segment = new Segment(track.source, init, msg.timestamp)

		// The track is responsible for flushing the segments in order
		track.source.initialize(init)
		track.add(segment)

		/* TODO I'm not actually sure why this code doesn't work; something trips up the MP4 parser
			while (1) {
				const data = await stream.read()
				if (!data) break

				segment.push(data)
				track.flush() // Flushes if the active segment has samples
			}
		*/

		// One day I'll figure it out; until then read one top-level atom at a time
		while (!await stream.done()) {
			const raw = await stream.peek(4)
			const size = new DataView(raw.buffer, raw.byteOffset, raw.byteLength).getUint32(0)
			const atom = await stream.bytes(size)

			segment.push(atom)
			track.flush() // Flushes if the active segment has new samples
		}

		segment.finish()
	}

	updateStats() {
		for (const child of this.statsRef.children) {
			if (child.className == "audio buffer") {
				const ranges: any = (this.audio) ? this.audio.buffered() : { length: 0 }
				this.visualizeBuffer(child as HTMLElement, ranges)
			} else if (child.className == "video buffer") {
				const ranges: any = (this.video) ? this.video.buffered() : { length: 0 }
				this.visualizeBuffer(child as HTMLElement, ranges)
			}
		}
	}

	visualizeBuffer(element: HTMLElement, ranges: TimeRanges) {
		const children = element.children
		const max = 5

		let index = 0
		let prev = 0

		for (let i = 0; i < ranges.length; i += 1) {
			let start = ranges.start(i) - this.vidRef.currentTime
			let end = ranges.end(i) - this.vidRef.currentTime

			if (end < 0 || start > max) {
				continue
			}

			let fill: HTMLElement;

			if (index < children.length) {
				fill = children[index] as HTMLElement;
			} else {
				fill = document.createElement("div")
				element.appendChild(fill)
			}

			fill.className = "fill"
			fill.innerHTML = end.toFixed(2)
			fill.setAttribute('style', "left: " + (100 * Math.max(start, 0) / max) + "%; right: " + (100 - 100 * Math.min(end, max) / max) + "%")
			index += 1

			prev = end
		}

		for (let i = index; i < children.length; i += 1) {
			element.removeChild(children[i])
		}
	}
}

// https://stackoverflow.com/questions/15900485/correct-way-to-convert-size-in-bytes-to-kb-mb-gb-in-javascript
function formatBits(bits: number, decimals: number = 1) {
	if (bits === 0) return '0 bits';

	const k = 1024;
	const dm = decimals < 0 ? 0 : decimals;
	const sizes = ['b', 'Kb', 'Mb', 'Gb', 'Tb', 'Pb', 'Eb', 'Zb', 'Yb'];

	const i = Math.floor(Math.log(bits) / Math.log(k));

	return parseFloat((bits / Math.pow(k, i)).toFixed(dm)) + ' ' + sizes[i];
}
