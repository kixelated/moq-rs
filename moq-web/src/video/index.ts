import { Watch } from "../watch";

const observedAttributes = ["src", "paused", "volume"] as const;
type ObservedAttribute = (typeof observedAttributes)[number];

// Supports a subset of the <video> element API.
// See: https://developer.mozilla.org/en-US/docs/Web/HTML/Element/video
// Also: https://developer.mozilla.org/en-US/docs/Web/API/HTMLMediaElement
export class Video extends HTMLElement implements HTMLVideoElement {
	#watch: Watch;

	// These only matter at init
	playsInline = true;
	poster = "";
	currentTime = 0;
	defaultMuted = false;
	preload: "" | "none" | "metadata" | "auto" = "auto";

	// Readonly properties.
	readonly buffered = emptyRange();
	readonly currentSrc = "";
	readonly duration = 0;
	readonly ended = false;
	readonly error = null;
	readonly played = emptyRange();
	readonly videoHeight = 0;
	readonly videoWidth = 0;

	readonly NETWORK_EMPTY = 0;
	readonly NETWORK_IDLE = 1;
	readonly NETWORK_LOADING = 2;
	readonly NETWORK_NO_SOURCE = 3;
	readonly networkState = this.NETWORK_EMPTY;

	readonly HAVE_NOTHING = 0;
	readonly HAVE_METADATA = 1;
	readonly HAVE_CURRENT_DATA = 2;
	readonly HAVE_FUTURE_DATA = 3;
	readonly HAVE_ENOUGH_DATA = 4;
	readonly readyState = this.HAVE_NOTHING;

	// Currently unsupported readonly properties
	readonly mediaKeys = null;
	readonly remote = emptyRemotePlayback();
	readonly textTracks = emptyTextTracks();
	readonly seekable = emptyRange();
	readonly seeking = false;
	readonly sinkId = "";

	// We don't care if this are set.
	disablePictureInPicture = false;
	onenterpictureinpicture = null;
	onleavepictureinpicture = null;
	controls = false;
	crossOrigin = null;
	disableRemotePlayback = true;
	onencrypted = null;
	onwaitingforkey = null;
	preservesPitch = true;

	static get unsupportedAttributes() {
		// Parts of the HTML5 <video> element that we don't support (yet), for documentation mostly.
		return [
			"controls",
			"controlslist",
			"crossorigin",
			"disablepictureinpicture",
			"disableremoteplayback",
			"loop",
			"playsinline",
			"poster",
		];
	}

	static get observedAttributes() {
		return observedAttributes;
	}

	// Attributes we only check once on init.
	static get initAttributes() {
		return ["autoplay"];
	}

	constructor() {
		super();

		this.#watch = new Watch();

		const shadow = this.attachShadow({ mode: "open" });
		shadow.appendChild(this.#watch);
	}

	connectedCallback() {
		for (const name of Video.observedAttributes) {
			const value = this.getAttribute(name);
			if (value !== null) {
				this.attributeChangedCallback(name, null, this.getAttribute(name));
			}
		}

		if (this.src) {
			this.load();
		}
	}

	disconnectedCallback() {}

	attributeChangedCallback(name: ObservedAttribute, oldValue: string | null, newValue: string | null) {
		if (oldValue === newValue) {
			return;
		}

		switch (name) {
			case "src":
				this.#watch.url = newValue || "";
				break;
			case "volume":
				this.#watch.volume = Number.parseFloat(newValue ?? "1");
				break;
			case "paused":
				this.#watch.paused = newValue !== null;
				break;
			default: {
				// Exhaustiveness check ensures all attributes are handled
				const _exhaustive: never = name;
				throw new Error(`Unhandled attribute: ${_exhaustive}`);
			}
		}
	}

	load() {
		// TODO wait until loading
	}

	pause(): void {
		this.setAttribute("paused", "");
		this.#watch.paused = true;
	}

	async play(): Promise<void> {
		this.removeAttribute("paused");
		this.#watch.paused = false;
		this.load();
	}

	get paused(): boolean {
		return this.getAttribute("paused") !== null;
	}

	// This width/height stuff is probably wrong
	get width(): number {
		const width = this.getAttribute("width");
		return width ? Number.parseInt(width) : this.offsetWidth;
	}

	set width(value: number) {
		this.setAttribute("width", value.toString());
	}

	get height() {
		const height = this.getAttribute("height");
		return height ? Number.parseInt(height) : this.offsetHeight;
	}

	set height(value: number) {
		this.setAttribute("height", value.toString());
	}

	get src(): string {
		return this.getAttribute("src") ?? "";
	}

	set src(value: string) {
		this.setAttribute("src", value);
	}

	get muted() {
		return this.getAttribute("muted") !== null;
	}

	set muted(value: boolean) {
		if (value) {
			this.setAttribute("muted", "");
		} else {
			this.removeAttribute("muted");
		}
	}

	get volume(): number {
		return Number.parseFloat(this.getAttribute("volume") ?? "1");
	}

	set volume(value: number) {
		this.setAttribute("volume", value.toString());
	}

	// Attrbutes that will error when changed (unsupported).
	get defaultPlaybackRate() {
		return 1;
	}

	set defaultPlaybackRate(value: number) {
		throw new Error("defaultPlaybackRate not implemented");
	}

	get autoplay(): boolean {
		return this.getAttribute("autoplay") !== null;
	}

	set autoplay(value: boolean) {
		if (value) {
			this.setAttribute("autoplay", "");
		} else {
			this.removeAttribute("autoplay");
		}
	}

	get loop() {
		return false;
	}

	set loop(value: boolean) {
		throw new Error("loop not implemented");
	}

	get playbackRate() {
		return 1;
	}

	set playbackRate(value: number) {
		throw new Error("playbackRate not implemented");
	}

	get srcObject() {
		return null;
	}

	set srcObject(value: MediaStream | MediaSource | Blob | null) {
		throw new Error("srcObject not implemented");
	}

	// Completely unsupported methods
	setMediaKeys(mediaKeys: MediaKeys | null): Promise<void> {
		throw new Error("Method not implemented.");
	}

	setSinkId(sinkId: string): Promise<void> {
		throw new Error("Method not implemented.");
	}

	requestVideoFrameCallback(callback: VideoFrameRequestCallback): number {
		throw new Error("Method not implemented.");
	}

	cancelVideoFrameCallback(handle: number): void {
		throw new Error("Method not implemented.");
	}

	getVideoPlaybackQuality(): VideoPlaybackQuality {
		throw new Error("Method not implemented.");
	}

	requestPictureInPicture(): Promise<PictureInPictureWindow> {
		throw new Error("Method not implemented.");
	}

	addTextTrack(kind: TextTrackKind, label?: string, language?: string): TextTrack {
		throw new Error("Method not implemented.");
	}

	canPlayType(type: string): CanPlayTypeResult {
		throw new Error("Method not implemented.");
	}

	fastSeek(time: number): void {
		throw new Error("Method not implemented.");
	}
}

function emptyRange(): TimeRanges {
	const invalid = () => {
		throw new RangeError("Index is not in the allowed range.");
	};

	return {
		length: 0,
		start: invalid,
		end: invalid,
	};
}

function emptyRemotePlayback(): RemotePlayback {
	return {
		onconnect: null,
		onconnecting: null,
		ondisconnect: null,
		state: "disconnected",
		cancelWatchAvailability: (id?: number): Promise<void> => {
			throw new Error("Function not implemented.");
		},
		prompt: (): Promise<void> => {
			throw new Error("Function not implemented.");
		},
		watchAvailability: (callback: RemotePlaybackAvailabilityCallback): Promise<number> => {
			throw new Error("Function not implemented.");
		},
		addEventListener: <K extends keyof RemotePlaybackEventMap>(
			type: K,
			// biome-ignore lint/suspicious/noExplicitAny: interface
			listener: (this: RemotePlayback, ev: RemotePlaybackEventMap[K]) => any,
			options?: boolean | AddEventListenerOptions,
		): void => {
			throw new Error("Function not implemented.");
		},
		removeEventListener: <K extends keyof RemotePlaybackEventMap>(
			type: K,
			// biome-ignore lint/suspicious/noExplicitAny: interface
			listener: (this: RemotePlayback, ev: RemotePlaybackEventMap[K]) => any,
			options?: boolean | EventListenerOptions,
		): void => {
			throw new Error("Function not implemented.");
		},
		dispatchEvent: (event: Event): boolean => {
			throw new Error("Function not implemented.");
		},
	};
}

function emptyTextTracks(): TextTrackList {
	return {
		length: 0,
		onaddtrack: null,
		onchange: null,
		onremovetrack: null,
		getTrackById: (id: string): TextTrack | null => {
			throw new Error("Function not implemented.");
		},
		addEventListener: <K extends keyof TextTrackListEventMap>(
			type: K,
			// biome-ignore lint/suspicious/noExplicitAny: <explanation>
			listener: (this: TextTrackList, ev: TextTrackListEventMap[K]) => any,
			options?: boolean | AddEventListenerOptions,
		): void => {
			throw new Error("Function not implemented.");
		},
		removeEventListener: <K extends keyof TextTrackListEventMap>(
			type: K,
			// biome-ignore lint/suspicious/noExplicitAny: <explanation>
			listener: (this: TextTrackList, ev: TextTrackListEventMap[K]) => any,
			options?: boolean | EventListenerOptions,
		): void => {
			throw new Error("Function not implemented.");
		},
		dispatchEvent: (event: Event): boolean => {
			throw new Error("Function not implemented.");
		},
	} as TextTrackList;
}

// Register the custom element.
customElements.define("moq-video", Video);

// Add our type to the typescript global scope.
declare global {
	interface HTMLElementTagNameMap {
		"moq-video": Video;
	}
}

export default Video;
