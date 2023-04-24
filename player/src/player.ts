import { Source } from "./source"
import { StreamReader, StreamWriter } from "./stream"
import { InitParser } from "./init"
import { Segment } from "./segment"
import { Track } from "./track"
import { Message, MessageInit, MessagePing, MessagePong, MessagePref, MessageSegment } from "./message"
import { dbStore } from './db';

///<reference path="./types/webtransport.d.ts"/>

export class Player {
	mediaSource: MediaSource;

	init: Map<string, InitParser>;
	audio: Track;
	video: Track;

	quic?: Promise<WebTransport>;
	api?: Promise<WritableStream>;
	url: string;
	started?: boolean;
	paused?: boolean;

	// References to elements in the DOM
	vidRef: HTMLVideoElement; // The video element itself
	statsRef: HTMLElement; // The stats div
	throttleRef: HTMLButtonElement; // The throttle button
	resolutionsRef: HTMLSelectElement;
	throttleDDLRef: HTMLSelectElement;
	continueStreamingRef: HTMLButtonElement; // Continue or pause streaming on the server side

	activeBWTestRef: HTMLButtonElement; // The active bw test button
	activeBWAsset: any; // {url}
	activeBWResetTimer: NodeJS.Timer | undefined;

	bufferLevel: Map<string, number>;

	throttleCount: number; // number of times we've clicked the button in a row

	interval?: NodeJS.Timer;
	activeBWTestTimer?: NodeJS.Timer;

	timeRef?: DOMHighResTimeStamp;

	// set to performance.now() when ping is sent and set to undefined when pong is received.
	pingStartTime: number | undefined;

	selectedResolution: string | undefined;

	lastSegmentTimestamp: number = -1; // the timestamp value of the last received segment
	serverBandwidth: number; // Kbps - comes from server in each segment in etp field
	tcRate: number; // Kbps - comes from server in each segment in tcRate field
	throughputs: Map<string, number>;
	supress_throughput_value: boolean;
	activeBWTestResult: number;
	activeBWTestInterval: number;
	lastActiveBWTestResult: number;

	chunkStats: any[] = [];
	totalChunkCount = 0; // video track chunk count

	logFunc: Function;
	testId: string;

	constructor(props: any) {
		this.vidRef = props.vid
		this.statsRef = props.stats
		this.resolutionsRef = props.resolutions
		this.throttleRef = props.throttle
		this.throttleDDLRef = props.throttleDDLRef
		this.continueStreamingRef = props.continueStreamingRef
		this.activeBWTestRef = props.activeBWTestRef
		this.activeBWAsset = props.activeBWAsset;
		this.throughputs = new Map();
		this.throttleCount = 0;
		this.url = props.url;
		this.activeBWTestInterval = props.activeBWTestInterval * 1000 || 0;

		this.logFunc = props.logger;
		this.testId = this.createTestId();

		this.bufferLevel = new Map();

		this.serverBandwidth = 0;
		this.tcRate = 0;
		this.supress_throughput_value = false;
		this.activeBWTestResult = 0;
		this.lastActiveBWTestResult = 0;

		this.mediaSource = new MediaSource()
		this.vidRef.src = URL.createObjectURL(this.mediaSource)

		this.init = new Map()
		this.audio = new Track(new Source(this.mediaSource));
		this.video = new Track(new Source(this.mediaSource));

		if (props.autoStart) {
			this.start();
		}
	}

	createTestId = () => {
		return 't_' + (new Date().toISOString().replace(/[-:]/g, '').replace(/\..+/g, '').replace('T', '_')) + '_' + Math.round(Math.random() * 10000);
	};

	saveResultBySecond = (name: string, value: number, second: number) => {
		dbStore.addResultEntry({ testId: this.testId, name, value, second });
	};

	start = async () => {
		// player can be started for once
		if (this.started) {
			return;
		}

		try {
			console.log('initing db');
			if (!await dbStore.init()) {
				console.log('db already inited');
			} else {
				console.log('db inited');
			}
		} catch (ex) {
			alert('db store could not be created');
			console.error(ex);
			return;
		}

		dbStore.addTestEntry({ testId: this.testId, timestamp: Date.now(), config: window.config });

		this.started = true;
		this.paused = (this.continueStreamingRef.dataset.status || 'streaming') === 'paused';

		this.interval = setInterval(this.tick.bind(this), 100)
		this.vidRef.addEventListener("waiting", this.tick.bind(this))

		this.resolutionsRef.addEventListener('change', this.resolutionOnChange)
		this.throttleDDLRef.addEventListener('change', this.throttleOnChange);
		this.continueStreamingRef.addEventListener('click', this.continueStreamingClicked);
		this.activeBWTestRef.addEventListener('click', this.startActiveBWTest);

		console.log('in start | url: %s', this.url);
		const quic = new WebTransport(this.url)
		this.quic = quic.ready.then(() => { return quic });

		// Create a unidirectional stream for all of our messages
		this.api = this.quic.then((q) => {
			return q.createUnidirectionalStream()
		})

		// async functions
		this.receiveStreams()

		if (this.activeBWTestInterval > 0) {
			setTimeout(() => {
				this.activeBWTestTimer = setInterval(() => this.startActiveBWTest(), this.activeBWTestInterval);
			}, this.activeBWTestInterval);
		}

		// Limit to 4Mb/s
		// this.sendThrottle()
	}

	stop = async () => {
		if (this.activeBWTestTimer) {
			clearInterval(this.activeBWTestTimer);
		}

		// reset tc netem limiting
		try {
			await this.sendMessage({
				"debug": {
					tc_reset: true,
				}
			});
		} finally {
			location.reload();
		}

	};

	pauseOrResume = (pause?: boolean) => {
		console.log('in pauseOrResume | paused: %s pause: %s', this.paused, pause);
		let status = this.continueStreamingRef.dataset.status || 'streaming';

		let sendMessage = false;

		if (!this.paused && (pause === true || pause === undefined)) {
			this.continueStreamingRef.innerText = 'Continue Streaming';
			this.paused = true;
			if (this.activeBWTestTimer) {
				clearInterval(this.activeBWTestTimer);
			}
			sendMessage = true;
		} else if (this.paused && !pause) {
			this.continueStreamingRef.innerText = 'Pause Streaming';
			this.paused = false;

			// re-schedule active bw test
			if (this.activeBWTestTimer) {
				clearInterval(this.activeBWTestTimer);
			}
			this.activeBWTestTimer = setInterval(() => this.startActiveBWTest(), this.activeBWTestInterval);
			sendMessage = true;
		}

		if (sendMessage) {
			this.continueStreamingRef.dataset.status = this.paused ? 'paused' : 'streaming';
			// send a debug message
			this.sendMessage({
				"debug": {
					"continue_streaming": !this.paused,
				}
			})
		}
	};

	getSWMAThreshold = () => {
		return window.config.swma_threshold || 5;
	}

	getSWMACalculationType = () => {
		return window.config.swma_calculation_type;
	}

	getSWMAThresholdType = () => {
		return window.config.swma_threshold_type || 'percentage'
	};

	getSWMACalculationInterval = () => {
		return window.config.swma_calculation_interval || 10;
	}

	getSWMAWindowSize = () => {
		return window.config.swma_window_size || 50;
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
		this.pauseOrResume();
	};

	startActiveBWTest = () => {
		console.log('in activeBWTestClicked', this.activeBWAsset)

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
						if (this.activeBWResetTimer) {
							clearTimeout(this.activeBWResetTimer)
						}
						this.activeBWResetTimer = setTimeout(() => { this.activeBWTestResult = 0; }, 1000);

						// don't display swmaThroughput threshold for a few seconds
						// to let the server warm-up
						this.supress_throughput_value = true;
						setTimeout(() => {
							this.supress_throughput_value = false;
						}, 1000);
					}
					this.activeBWTestRef.dataset.downloading = '';
					this.activeBWTestRef.innerText = 'Active BW Test';

					if (this.paused) {
						this.continueStreamingRef.click();
					}
				}).catch(e => {
					console.error('Error in downloading 2', e);
				});
			}, delay);
		};

		// if the streaming is active, first stop it and then start test
		if (!this.paused) {
			this.continueStreamingRef.click();
			test(3000);
		} else {
			// start without waiting
			test(0);
		}
	};

	async close() {
		if (!this.quic) {
			return;
		}
		clearInterval(this.interval);
		(await this.quic).close()
	}

	sendPreference = async (pref: MessagePref) => {
		console.info('sending preference', pref);
		await this.sendMessage({ 'x-pref': pref });
	};

	async sendMessage(msg: any) {
		if (!this.api) {
			return;
		}

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
		if (!this.quic) {
			return;
		}

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

		// since streams are multiplexed
		// a stale segment may come later which changes the latest
		// etp and tc_rate values inadvertently.
		if (msg.timestamp >= this.lastSegmentTimestamp) {
			this.serverBandwidth = msg.etp * 1024; // in bits, comes as Kbps
			this.tcRate = msg.tc_rate * 1024; // in bits, comes as Kbps
		}
		this.lastSegmentTimestamp = msg.timestamp;

		console.log('msg: %o tcRate: %d serverBandwidth: %d', msg, this.tcRate, this.serverBandwidth)

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

		let totalSegmentSize = 0;
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

					++this.totalChunkCount;

					dbStore.addLogEntry({
						testId: this.testId,
						segmentId: msg.init,
						no: chunkCounter,
						chunkSize,
						chunkDownloadDuration,
						lastMoofDownloadDuration,
						chunkLatency,
						msg_timestamp: msg.timestamp,
						msg_at: msg.at,
						msg_etp: msg.etp,
						msg_tc_rate: msg.tc_rate,
						perf_now: performance.now(),
						activeBWTestResult: this.activeBWTestResult,
						lastActiveBWTestResult: this.lastActiveBWTestResult,
						timestamp: Date.now()
					});

					const stat = [chunkCounter, chunkSize, chunkDownloadDuration, lastMoofDownloadDuration, chunkDownloadDuration > 0 ? (chunkSize * 8 * 1000 / chunkDownloadDuration) : 0, chunkLatency, msg.timestamp];
					this.chunkStats.push(stat);

					if (chunkCounter === 1) {
						let filteredStats = [stat];
						const val = this.computeTPut(filteredStats);
						console.log('ifa calc', val, stat, this.throughputs.get('ifa'));
						// if the value is an outlier (100 times more than the last measurement)
						// discard it
						const lastVal = (this.throughputs.get('ifa') || 0);
						if (lastVal === 0 || val < lastVal * 100) {
							this.throughputs.set('ifa', val);
						}
					}

					if (this.totalChunkCount >= this.getSWMAWindowSize() && this.totalChunkCount % this.getSWMACalculationInterval() === 0) {
						const stats = this.chunkStats.slice(-this.getSWMAWindowSize());
						let filteredStats: any[] = this.filterStats(stats, this.getSWMAThreshold(), this.getSWMAThresholdType(), this.throughputs.get('swma') || 0);
						const tput = this.computeTPut(filteredStats);
						if (tput > 0) {
							this.throughputs.set('swma', tput);
						} else {
							console.warn('tput is zero.');
						}
						
					}
				}
			}
			totalSegmentSize += size;

			segment.push(atom)

			track.flush() // Flushes if the active segment has new samples
		}

		segment.finish()

		const segmentFinish = performance.now() - segmentDownloadStart;

		if (isVideoSegment) {
			this.logFunc('-----------------------------------------------------')
			this.logFunc('segment chunk length: ' + chunkCounter);
			this.logFunc('segment finish duration: ' + Math.round(segmentFinish));
			this.logFunc('total segment size: ' + formatBits(totalSegmentSize * 8));
			this.logFunc('segment start (client): ' + new Date(performance.timeOrigin + segmentStartOffset).toISOString());
			this.logFunc('availability time (server): ' + new Date(msg.at).toISOString());
		}
	}

	logChunkStats = (filteredChunkStats: any[]) => {
		this.logFunc('-----------------------------------------------------')
		this.logFunc('swma calculation type: ' + this.getSWMACalculationType());
		if (this.getSWMACalculationType() === 'window') {
			this.logFunc('swma window size: ' + this.getSWMAWindowSize());
			this.logFunc('calculation interval: ' + this.getSWMACalculationInterval());
		}
		this.logFunc('total number of chunks: ' + this.totalChunkCount);
		this.logFunc('')
		this.logFunc('swma: ' + formatBits(this.throughputs.get('swma') || 0));
		this.logFunc('ifa: ' + formatBits(this.throughputs.get('ifa') || 0));
		this.logFunc('number of discarded chunks: ' + (this.chunkStats.length - filteredChunkStats.length));
		this.logFunc('')
		this.logFunc('#\tChunk Size(byte)\tMDat Download Duration(ms)\tMoof Download Duration\tDownload Rate\tAvailability Offset (ms)\tSegment TS');
		this.chunkStats.forEach((row: any) => {
			this.logFunc(row.join('\t'));
		});
		this.logFunc('-----------------------------------------------------');
	}

	filterStats = (chunkStats: any[], threshold: number, thresholdType: string, lastTPut?: number) => {
		let filteredStats = chunkStats.slice();
		console.log('computeTPut | chunk count: %d thresholdType: %s threshold: %d', filteredStats.length, thresholdType, threshold);

		let zeroDurations = filteredStats.filter(a => a[2] === 0);
		filteredStats = filteredStats.filter(a => a[2] > 0);

		if (thresholdType === 'percentage') {
			if (threshold > 0 && threshold < 100) {
				// sort chunk by download rate, in descending order
				filteredStats.sort((a, b) => {
					return (a[2] ? a[1] / a[2] : 0) > (b[2] ? b[1] / b[2] : 0) ? -1 : 1;
				});

				const topCut = Math.ceil(threshold / 100 * filteredStats.length);
				const bottomCut = Math.floor(threshold / 100 * filteredStats.length);

				filteredStats.splice(0, topCut);
				filteredStats.splice(filteredStats.length - bottomCut, bottomCut);
			}
		} else if (thresholdType === 'minimum_duration') {
			filteredStats = filteredStats.filter(c => c[2] >= threshold);
		}

		filteredStats = filteredStats.concat(zeroDurations);

		console.log('computeTPut | after filtering: chunk count: %d', filteredStats.length);
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
			bw_swma_threshold.innerText = formatBits(this.throughputs.get('swma') || 0, 1).toString() + ' / ' + formatBits(this.throughputs.get('ifa') || 0, 1).toString();
			bw_active_bw.innerText = formatBits(this.lastActiveBWTestResult, 1).toString();
		}
	}

	getDbStore = () => {
		return dbStore;
	};

	downloadStats = async (testId?: string) => {
		console.log('in downloadStats');

		const link = document.createElement('a');
		document.body.appendChild(link);

		// download logs


		const logs = await dbStore.getLogs(testId || this.testId);
		if (logs.length > 0) {
			const headers = Object.keys(logs[0]);
			const csvContent = 'data:application/vnd.ms-excel;charset=utf-8,' + headers.join('\t') + '\n' + logs.map(e => Object.values(e).join('\t')).join('\n');
			const encodedUri = encodeURI(csvContent);
			link.setAttribute('href', encodedUri);
			link.setAttribute('download', 'logs_' + this.testId + '.xls');
			link.click();
		} else {
			console.log('no logs');
		}

		// download results
		const results = await dbStore.getResults(testId || this.testId);
		if (results.length > 0) {
			const headers = Object.keys(results[0]);
			const csvContent = 'data:application/vnd.ms-excel;charset=utf-8,' + headers.join('\t') + '\n' + results.map(e => Object.values(e).join('\t')).join('\n');
			const encodedUri = encodeURI(csvContent);
			link.setAttribute('href', encodedUri);
			link.setAttribute('download', 'results_' + this.testId + '.xls');
			link.click();
		} else {
			console.log('no results');
		}

		link.remove();
	};

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