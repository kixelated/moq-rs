import { Source } from "./source"
import { StreamReader, StreamWriter } from "./stream"
import { InitParser } from "./init"
import { Segment } from "./segment"
import { Track } from "./track"
import { Message, MessageInit, MessagePing, MessagePong, MessagePref, MessageSegment } from "./message"

///<reference path="./types/webtransport.d.ts"/>

export class Player {
	mediaSource: MediaSource;

	init: Map<string, InitParser>;
	audio: Track;
	video: Track;

	quic: Promise<WebTransport>;
	api: Promise<WritableStream>;

	// References to elements in the DOM
	vidRef: HTMLVideoElement; // The video element itself
	statsRef: HTMLElement; // The stats div
	throttleRef: HTMLButtonElement; // The throttle button
	resolutionsRef: HTMLSelectElement;
	throttleDDLRef: HTMLSelectElement;
	continueStreamingRef: HTMLButtonElement; // Continue or pause streaming on the server side

	activeBWTestRef: HTMLButtonElement; // The active bw test button
	activeBWAsset: any; // {url}
	activeBWBWResetTimer: NodeJS.Timer | undefined;

	bufferLevel: Map<string, number>;

	throttleCount: number; // number of times we've clicked the button in a row

	interval: NodeJS.Timer;

	timeRef?: DOMHighResTimeStamp;

	// set to performance.now() when ping is sent and set to undefined when pong is received.
	pingStartTime: number | undefined;

	selectedResolution: string | undefined;

	serverBandwidth: number; // Kbps - comes from server in each segment in etp field
	swmaThroughput_threshold: number;
	activeBWTestResult: number;
	lastActiveBWTestResult: number;

	logFunc: Function;

	constructor(props: any) {
		this.vidRef = props.vid
		this.statsRef = props.stats
		this.resolutionsRef = props.resolutions
		this.throttleRef = props.throttle
		this.throttleDDLRef = props.throttleDDLRef
		this.continueStreamingRef = props.continueStreamingRef
		this.activeBWTestRef = props.activeBWTestRef
		this.activeBWAsset = props.activeBWAsset;

		this.throttleCount = 0

		this.logFunc = props.logger;

		this.bufferLevel = new Map();

		this.serverBandwidth = 0;
		this.swmaThroughput_threshold = 0;
		this.activeBWTestResult = 0;
		this.lastActiveBWTestResult = 0;

		this.mediaSource = new MediaSource()
		this.vidRef.src = URL.createObjectURL(this.mediaSource)

		this.init = new Map()
		this.audio = new Track(new Source(this.mediaSource));
		this.video = new Track(new Source(this.mediaSource));

		this.interval = setInterval(this.tick.bind(this), 100)
		this.vidRef.addEventListener("waiting", this.tick.bind(this))

		this.resolutionsRef.addEventListener('change', this.resolutionOnChange)
		this.throttleDDLRef.addEventListener('change', this.throttleOnChange);
		this.continueStreamingRef.addEventListener('click', this.continueStreamingClicked);
		this.activeBWTestRef.addEventListener('click', this.activeBWTestClicked);

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

	resolutionOnChange = () => {
		const selectedResolution = this.resolutionsRef.options[this.resolutionsRef.selectedIndex];
		console.log('in resolutionOnChange | resolution: %s', selectedResolution.value);

		if (selectedResolution.value.length > 0) {
			this.selectedResolution = selectedResolution.value;
			const resolutionPreference: MessagePref = {
				name: 'resolution',
				value: this.selectedResolution
			};
			this.sendPreference(resolutionPreference);
		}
	};

	throttleOnChange = () => {
		let rate = 0;

		if (this.throttleDDLRef.options.selectedIndex > 0) {
			rate = parseInt(this.throttleDDLRef.options[this.throttleDDLRef.options.selectedIndex].value);
		}

		if (rate === 0) {
			return;
		}

		// Send the server a message to fake network congestion.
		this.sendMessage({
			"debug": {
				max_bitrate: rate,
			},
		})
	};

	continueStreamingClicked = () => {
		let status = this.continueStreamingRef.dataset.status || 'streaming';

		if (status === 'streaming') {
			this.continueStreamingRef.innerText = 'Continue Streaming';
			status = 'stopped';
		} else {
			this.continueStreamingRef.innerText = 'Stop Streaming';
			status = 'streaming';
		}

		this.continueStreamingRef.dataset.status = status;

		// send a debug message
		this.sendMessage({
			"debug": {
				"continue_streaming": status === 'streaming',
			}
		})
	};

	activeBWTestClicked = () => {
		console.log('in activeBWTestClicked | activeBWAsset: %s', this.activeBWAsset)

		if (!this.activeBWAsset) {
			return;
		}

		if (this.activeBWTestRef.dataset.downloading === 'true') {
			return;
		}

		let { url } = this.activeBWAsset;

		if (!url) {
			return;
		} else {
			url += "?r=" + Math.random();
		}


		const test = (delay: number) => {
			this.activeBWTestRef.dataset.downloading = 'true';
			this.activeBWTestRef.innerText = 'Downloading...';

			setTimeout(() => {
				const start = performance.now();
				fetch(url, { cache: 'no-store' }).then((resp: Response) => {
					if (!resp.ok) {
						console.error('Error in downloading 1', resp.status, resp.statusText);
					} else {
						return resp.blob();
					}
				}).then((resp) => {
					const end = performance.now();
					const duration = end - start;
					const size = resp?.size;
					if (!size) {
						console.error('Active bandwidth test asset size is 0');
					} else {
						console.info('started: %d ended: %d | duration: %d', start, end, duration, resp);
						this.activeBWTestResult = Math.round(size * 8 * 1000 / duration);
						this.lastActiveBWTestResult = this.activeBWTestResult;
						// if another timer is active, clear it
						if (this.activeBWBWResetTimer) {
							clearTimeout(this.activeBWBWResetTimer)
						}
						this.activeBWBWResetTimer = setTimeout(() => { this.activeBWTestResult = 0; }, 1000);

					}
					this.activeBWTestRef.dataset.downloading = '';
					this.activeBWTestRef.innerText = 'Active BW Test';

					const status = this.continueStreamingRef.dataset.status || 'streaming';
					if (status == 'stopped') {
						this.continueStreamingRef.click();
					}
				}).catch(e => {
					console.error('Error in downloading 2', e);
				});
			}, delay);
		};

		// if the streaming is active, first stop it and then start test
		const status = this.continueStreamingRef.dataset.status || 'streaming';
		if (status == 'streaming') {
			this.continueStreamingRef.click();
			test(3000);
		} else {
			// start without waiting
			test(0);
		}
	};

	async close() {
		clearInterval(this.interval);
		(await this.quic).close()
	}

	sendPreference = async (pref: MessagePref) => {
		console.info('sending preference', pref);
		await this.sendMessage({ 'x-pref': pref });
	};

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


	ping() {
		// a ping already made
		if (this.pingStartTime) {
			return;
		}
		this.pingStartTime = performance.now();
		this.sendPing();
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
		let rate = 0;

		if (this.throttleCount > 0) {
			// TODO detect the incoming bitrate instead of hard-coding
			// Right shift by throttle to divide by 2,4,8,16,etc each time
			const bitrate = 4 * 1024 * 1024 // 4Mb/s

			rate = bitrate >> (this.throttleCount - 1)

			const str = formatBits(rate) + "/s"
			this.throttleRef.textContent = `Throttle: ${str}`;
		} else {
			this.throttleRef.textContent = "Throttle: none";
		}

		// Send the server a message to fake network congestion.
		this.sendMessage({
			"debug": {
				max_bitrate: rate,
			},
		})
	}


	sendPing() {
		this.sendMessage({
			"x-ping": {

			}
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

		this.vidRef.currentTime = ranges.end(ranges.length - 1);
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
		let counter = 0;
		const q = await this.quic

		const streams = q.incomingUnidirectionalStreams.getReader();

		while (true) {
			++counter;
			const result = await streams.read();

			if (result.done) break

			const stream = result.value
			this.handleStream(stream) // don't await
		}
	}

	async handleStream(stream: ReadableStream) {
		let r = new StreamReader(stream.getReader())
		while (true) {
			const start = performance.now();

			if (await r.done()) {
				break;
			}

			const size = await r.uint32();
			const typ = new TextDecoder('utf-8').decode(await r.bytes(4));
			if (typ !== "warp") throw "expected warp atom"
			if (size < 8) throw "atom too small"

			const payload = new TextDecoder('utf-8').decode(await r.bytes(size - 8));
			const msg = JSON.parse(payload) as Message

			if (msg.init) {
				return this.handleInit(r, msg.init)
			} else if (msg.segment) {
				return this.handleSegment(r, msg.segment, start)
			} else if (msg.pong) {
				return this.handlePong(r, msg.pong)
			}
		}
	}

	// TODO: time-sync should be made for this to give correct result
	async handlePong(stream: StreamReader, msg: MessagePong) {
		if (!this.pingStartTime) {
			console.warn('in handlePong | pingStartTime is undefined.');
			return;
		}
		const latency = performance.now() - this.pingStartTime;
		console.log('Latency is: %d ms', latency);
		this.pingStartTime = undefined;
	}

	async handleInit(stream: StreamReader, msg: MessageInit) {
		let init = this.init.get(msg.id);
		if (!init) {
			init = new InitParser()
			this.init.set(msg.id, init)
		}

		while (1) {
			const data = await stream.read()
			//request arrived
			if (!data) break
			init.push(data)
		}
	}

	async handleSegment(stream: StreamReader, msg: MessageSegment, segmentStartOffset: number) {
		let initParser = this.init.get(msg.init);
		if (!initParser) {
			initParser = new InitParser()
			this.init.set(msg.init, initParser)
		}

		// Wait for the init segment to be fully received and parsed
		const init = await initParser.ready;
		//request arrived
		let track: Track;
		if (init.info.videoTracks.length) {
			track = this.video
		} else {
			track = this.audio
		}

		this.serverBandwidth = msg.etp * 1024; // in bits, etp comes in kbit

		const segment = new Segment(track.source, init, msg.timestamp)
		// The track is responsible for flushing the segments in order
		track.add(segment)

		/* TODO I'm not actually sure why this code doesn't work; something trips up the MP4 parser
			while (1) {
				const data = await stream.read()
				if (!data) break

				segment.push(data)
				track.flush() // Flushes if the active segment has samples
			}
		*/

		// calculate the bandwidth using sliding window moving average
		const chunkStats = [];
		let totalSegmentSize = 0;
		let totalChunkCount = 0;
		const segmentDownloadStart = performance.now()

		let chunkCounter = 0;
		let isVideoSegment = init.info.videoTracks.length > 0;
		let lastMoofSize = 0;
		let lastMoofStartTime = 0;
		let lastMoofDownloadDuration = 0;
		let lastMoofClockTime = 0;

		// One day I'll figure it out; until then read one top-level atom at a time
		while (true) {
			if (await stream.done()) {
				console.log('end of stream')
				break;
			}

			const boxStartOffset = performance.now();

			const raw = await stream.peek(4)
			const size = new DataView(raw.buffer, raw.byteOffset, raw.byteLength).getUint32(0)
			const atom = await stream.bytes(size)

			// boxes: [moof][mdat]...<idle time>...[moof][mdat]
			// first 4 bytes => size
			// following 4 bytes => box type
			const boxType = fromCharCodeUint8([...atom.slice(4, 8)]);

			if (isVideoSegment) {
				if (boxType === 'moof') {
					chunkCounter++;
					lastMoofSize = size;
					lastMoofStartTime = boxStartOffset;
					lastMoofDownloadDuration = performance.now() - lastMoofStartTime;
					lastMoofClockTime = Date.now();
				} else if (boxType === 'mdat') {
					const chunkDownloadDuration = performance.now() - lastMoofStartTime;
					const chunkSize = size + lastMoofSize; // bytes
					const chunkLatency = Math.round(lastMoofClockTime - msg.at);

					chunkStats.push([chunkCounter, chunkSize, chunkDownloadDuration, lastMoofDownloadDuration, chunkDownloadDuration > 0 ? (chunkSize * 8 * 1000 / chunkDownloadDuration) : 0, chunkLatency]);

					++totalChunkCount;
				}
			}
			totalSegmentSize += size;

			segment.push(atom)

			track.flush() // Flushes if the active segment has new samples
		}

		segment.finish()

		const segmentFinish = performance.now() - segmentDownloadStart;

		if (isVideoSegment) {
			let filteredStats: any[] = this.filterStats(chunkStats, window.config.swma_threshold || 5, window.config.swma_threshold_type || 'percentage');
			this.swmaThroughput_threshold = this.computeTPut(filteredStats);

			this.logFunc('-----------------------------------------------------')
			this.logFunc('segment chunk length: ' + chunkStats.length);
			this.logFunc('segment finish duration: ' + Math.round(segmentFinish))
			this.logFunc('total segment size: ' + formatBits(totalSegmentSize * 8))
			this.logFunc('segment start (client): ' + new Date(performance.timeOrigin + segmentStartOffset).toISOString());
			this.logFunc('availability time (server): ' + new Date(msg.at).toISOString())
			this.logFunc('first chunk offset (wrt. at): ' + chunkStats[0][5]);

			this.logFunc('swmaThroughput_threshold: ' + formatBits(this.swmaThroughput_threshold));
			this.logFunc('number of discarded chunks: ' + (chunkStats.length - filteredStats.length));

			this.logFunc('')
			this.logFunc('#\tChunk Size(byte)\tMDat Download Duration(ms)\tMoof Download Duration\tDownload Rate\tAvailability Offset (ms)');
			chunkStats.forEach(row => {
				this.logFunc(row.join('\t'));
			});
			this.logFunc(('-----------------------------------------------------'));
			this.logFunc('');
		}
	}

	filterStats = (chunkStats: any[], threshold: number, thresholdType: string) => {
		let filteredStats = chunkStats.slice();
		if (thresholdType === 'percentage') {
			if (threshold > 0 && threshold < 100) {
				// sort chunk by download rate, in descending order
				filteredStats.sort((a, b) => {
					return (a[2] ? a[1] / a[2] : 0) > (b[2] ? b[1] / b[2] : 0) ? -1 : 1;
				});

				const topCut = Math.ceil(threshold / 100 * filteredStats.length);
				const bottomCut = Math.floor(threshold / 100 * filteredStats.length);

				console.log('computeTPut | chunk count: %d top cut: %d bottom cut: %d', filteredStats.length, topCut, bottomCut);
				filteredStats.splice(0, topCut);
				filteredStats.splice(filteredStats.length - bottomCut, bottomCut);
				console.log('computeTPut | after filtering: chunk count: %d', filteredStats.length);
			}
		} else if (thresholdType === 'minimum_duration') {
			console.log('computeTPut | chunk count: %d minimum duration: %d', filteredStats.length, threshold);
			filteredStats = filteredStats.filter(c => c[2] >= threshold);
			console.log('computeTPut | after filtering: chunk count: %d', filteredStats.length);
		}
		return filteredStats;
	}

	computeTPut = (stats: any[]) => {
		let totalSize = 0;
		let totalDuration = 0;
		stats.forEach((arr, i) => {
			const size = arr[1];
			const downloadDurationOfChunk = arr[2];
			if (size > 0 && downloadDurationOfChunk > 0) {
				totalSize += size;
				totalDuration += downloadDurationOfChunk;
			}
		});
		return totalSize * 8 * 1000 / totalDuration;
	};

	updateStats = () => {
		const audioFiller = this.statsRef.querySelector('.audio.buffer .fill') as HTMLElement;
		const audioBufferDurationEl = this.statsRef.querySelector('.audio.label>.seconds') as HTMLElement;
		if (audioFiller) {
			const ranges: any = (this.audio) ? this.audio.buffered() : { length: 0 };
			this.visualizeBuffer(audioFiller, audioBufferDurationEl, 'audio', ranges);
		}

		const videoFiller = this.statsRef.querySelector('.video.buffer .fill') as HTMLElement;
		const videoBufferDurationEl = this.statsRef.querySelector('.video.label>.seconds') as HTMLElement;
		if (videoFiller) {
			const ranges: any = (this.video) ? this.video.buffered() : { length: 0 }
			this.visualizeBuffer(videoFiller, videoBufferDurationEl, 'video', ranges)
		}

		const bw = document.querySelector('#stats .server_bw') as HTMLDivElement;
		const bw_swma_threshold = document.querySelector('#stats .swma_threshold') as HTMLDivElement;
		const bw_active_bw = document.querySelector('#stats .active_bw') as HTMLDivElement;


		if (bw) {
			bw.innerText = formatBits(this.serverBandwidth, 1).toString();
			bw_swma_threshold.innerText = formatBits(this.swmaThroughput_threshold, 1).toString();
			bw_active_bw.innerText = formatBits(this.lastActiveBWTestResult, 1).toString();
		}
	}

	visualizeBuffer(bufferFiller: HTMLElement, durationEl: HTMLElement, bufferType: 'audio' | 'video', ranges: TimeRanges) {
		const max = 5

		let index = 0
		let prev = 0

		// TODO: check buffer calculation
		for (let i = 0; i < ranges.length; i += 1) {
			let start = ranges.start(i) - this.vidRef.currentTime
			let end = ranges.end(i) - this.vidRef.currentTime

			if (end < 0 || start > max) {
				continue
			}
			this.bufferLevel.set(bufferType, end);
			durationEl.innerText = end.toFixed(2)
			bufferFiller.setAttribute('style', "left: " + (100 * Math.max(start, 0) / max) + "%; right: " + (100 - 100 * Math.min(end, max) / max) + "%")
			index += 1

			prev = end
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

function fromCharCodeUint8(uint8arr: any[]) {
	var arr = [];
	for (var i = 0; i < uint8arr.length; i++) {
		arr[i] = uint8arr[i];
	}
	return String.fromCharCode.apply(null, arr);
}