import { JSX, Match, Show, Switch, createMemo, createSelector, createSignal } from "solid-js";

export type SupportRole = "core" | "watch" | "publish" | "all";
export type SupportShow = "full" | "partial" | "none";

export type SupportAudio = {
	aac: boolean;
	opus: boolean;
};

export type SupportCodec = {
	hardware: boolean;
	software: boolean;
};

export type SupportVideo = {
	h264: SupportCodec;
	h265: SupportCodec;
	vp8: SupportCodec;
	vp9: SupportCodec;
	av1: SupportCodec;
};

export type Support = {
	webtransport: boolean;
	audio: {
		encoding: SupportAudio | undefined;
		decoding: SupportAudio | undefined;
	};
	video: {
		encoding: SupportVideo | undefined;
		decoding: SupportVideo | undefined;
	};
};

// Pick a codec string for each codec.
// This is not strictly correct, as browsers may not support every profile or level.
const CODECS = {
	aac: "mp4a.40.2",
	opus: "opus",
	av1: "av01.0.08M.08",
	h264: "avc1.640028",
	h265: "hev1.1.6.L93.B0",
	vp9: "vp09.00.10.08",
	vp8: "vp8",
};

async function audioDecoderSupported(codec: keyof typeof CODECS) {
	return (
		(
			await AudioDecoder.isConfigSupported({
				codec: CODECS[codec],
				numberOfChannels: 2,
				sampleRate: 48000,
			})
		).supported === true
	);
}

async function audioEncoderSupported(codec: keyof typeof CODECS) {
	return (
		(
			await AudioEncoder.isConfigSupported({
				codec: CODECS[codec],
				numberOfChannels: 2,
				sampleRate: 48000,
			})
		).supported === true
	);
}

async function videoDecoderSupported(codec: keyof typeof CODECS) {
	const software =
		(
			await VideoDecoder.isConfigSupported({
				codec: CODECS[codec],
				hardwareAcceleration: "prefer-software",
			})
		).supported === true;

	const hardware =
		(
			await VideoDecoder.isConfigSupported({
				codec: CODECS[codec],
				hardwareAcceleration: "prefer-hardware",
			})
		).supported === true;

	return {
		hardware,
		software,
	};
}

async function videoEncoderSupported(codec: keyof typeof CODECS) {
	const software =
		(
			await VideoEncoder.isConfigSupported({
				codec: CODECS[codec],
				width: 1280,
				height: 720,
				hardwareAcceleration: "prefer-software",
			})
		).supported === true;

	const hardware =
		(
			await VideoEncoder.isConfigSupported({
				codec: CODECS[codec],
				width: 1280,
				height: 720,
				hardwareAcceleration: "prefer-hardware",
			})
		).supported === true;

	return {
		hardware,
		software,
	};
}

export async function isSupported(): Promise<Support> {
	return {
		webtransport: typeof WebTransport !== "undefined",
		audio: {
			encoding:
				typeof AudioEncoder !== "undefined"
					? {
							aac: await audioEncoderSupported("aac"),
							opus: await audioEncoderSupported("opus"),
						}
					: undefined,
			decoding:
				typeof AudioDecoder !== "undefined"
					? {
							aac: await audioDecoderSupported("aac"),
							opus: await audioDecoderSupported("opus"),
						}
					: undefined,
		},
		video: {
			encoding:
				typeof VideoEncoder !== "undefined"
					? {
							h264: await videoEncoderSupported("h264"),
							h265: await videoEncoderSupported("h265"),
							vp8: await videoEncoderSupported("vp8"),
							vp9: await videoEncoderSupported("vp9"),
							av1: await videoEncoderSupported("av1"),
						}
					: undefined,
			decoding:
				typeof VideoDecoder !== "undefined"
					? {
							h264: await videoDecoderSupported("h264"),
							h265: await videoDecoderSupported("h265"),
							vp8: await videoDecoderSupported("vp8"),
							vp9: await videoDecoderSupported("vp9"),
							av1: await videoDecoderSupported("av1"),
						}
					: undefined,
		},
	};
}

export function Support(props: { role: SupportRole; show: SupportShow }) {
	const [support, setSupport] = createSignal<Support | undefined>();
	isSupported().then(setSupport);

	const core = createMemo<"full" | "none" | undefined>(() => {
		const s = support();
		if (!s) return;

		if (!s.webtransport) return "none";
		return "full";
	});

	const watch = createMemo<"full" | "partial" | "none" | undefined>(() => {
		const s = support();
		if (!s) return;

		if (!s.audio.decoding || !s.video.decoding) return "none";

		// Make sure we support decoding at least one codec of each type...
		if (!Object.values(s.audio.decoding).some((v) => v)) return "none";
		if (!Object.values(s.video.decoding).some((v) => v.software || v.hardware)) return "none";

		// Check if we support decoding all codecs.
		if (!Object.values(s.audio.decoding).every((v) => v)) return "partial";
		if (!Object.values(s.video.decoding).every((v) => v.software || v.hardware)) return "partial";

		return "full";
	});

	const publish = createMemo<"full" | "partial" | "none" | undefined>(() => {
		const s = support();
		if (!s) return;

		if (!s.audio.encoding || !s.video.encoding) return "none";

		// Make sure that we support encoding at least one codec of each type...
		if (!Object.values(s.audio.encoding).some((v) => v)) return "none";
		if (!Object.values(s.video.encoding).some((v) => v.software || v.hardware)) return "none";

		// Make sure we support encoding at least one codec with hardware acceleration.
		if (!Object.values(s.video.encoding).some((v) => v.hardware)) return "partial";

		return "full";
	});

	const final = createMemo<"full" | "partial" | "none" | undefined>(() => {
		const b = core();
		if (b === "none" || props.role === "core") return b;

		if (props.role === "watch") {
			return watch();
		}

		if (props.role === "publish") {
			return publish();
		}

		const w = watch();
		const p = publish();

		if (w === "none" || p === "none") return "none";
		if (w === "partial" && p === "partial") return "partial";

		return "full";
	});

	const isFinal = createSelector(final);
	const [showDetails, setShowDetails] = createSignal<boolean>(false);

	// Only render based on the result.
	const shouldShow = () => {
		if (props.show === "full") return true;
		if (props.show === "partial") return isFinal("partial") || isFinal("none");
		return isFinal("none");
	};

	return (
		<Show when={shouldShow()}>
			<div style={{ margin: "0 auto", "max-width": "28rem", padding: "1rem" }}>
				<div
					style={{
						display: "flex",
						"flex-direction": "row",
						gap: "1rem",
						"flex-wrap": "wrap",
						"justify-content": "space-between",
						"align-items": "center",
					}}
				>
					<div style={{ "font-weight": "bold" }}>
						<Switch>
							<Match when={isFinal("full")}>🟢 Full Browser Support</Match>
							<Match when={isFinal("partial")}>🟡 Partial Browser Support</Match>
							<Match when={isFinal("none")}>🔴 No Browser Support</Match>
						</Switch>
					</div>
					<button type="button" onClick={() => setShowDetails((d) => !d)} style={{ "font-size": "14px" }}>
						{showDetails() ? "Hide Details ➖" : "Show Details ➕"}
					</button>
				</div>
				<Show when={showDetails()}>
					<SupportDetails support={support()} role={props.role} />
				</Show>
			</div>
		</Show>
	);
}

const SupportDetails = (props: { support: Support | undefined; role: "core" | "watch" | "publish" | "all" }) => {
	const support = props.support;
	if (!support) return null;

	const c1: JSX.CSSProperties = {
		"grid-column-start": 1,
		"font-weight": "bold",
		"text-align": "right",
	};

	const c2: JSX.CSSProperties = {
		"grid-column-start": 2,
		"text-align": "center",
	};

	const c3 = {
		"grid-column-start": 3,
	};

	const binary = (value: boolean | undefined) => (value ? "🟢 Yes" : "🔴 No");
	const hardware = (codec: SupportCodec | undefined) =>
		codec?.hardware ? "🟢 Hardware" : codec?.software ? "🟡 Software" : "🔴 No";

	return (
		<div
			style={{
				display: "grid",
				"grid-template-columns": "1fr 1fr 1fr",
				"column-gap": "0.5rem",
				"row-gap": "0.2rem",
				"background-color": "rgba(255, 255, 255, 0.25)",
				"border-radius": "0.5rem",
				padding: "1rem",
				"font-size": "0.875rem",
			}}
		>
			<div style={c1}>WebTransport</div>
			<div style={c3}>{binary(support.webtransport)}</div>
			<Show when={props.role !== "core"}>
				<Show when={props.role !== "watch"}>
					<div style={c1}>Encoding</div>
					<div style={c2}>Opus</div>
					<div style={c3}>{binary(support.audio.encoding?.opus)}</div>
					<div style={c2}>AAC</div>
					<div style={c3}>{binary(support.audio.encoding?.aac)}</div>
					<div style={c2}>AV1</div>
					<div style={c3}>{hardware(support.video.encoding?.av1)}</div>
					<div style={c2}>H.265</div>
					<div style={c3}>{hardware(support.video.encoding?.h265)}</div>
					<div style={c2}>H.264</div>
					<div style={c3}>{hardware(support.video.encoding?.h264)}</div>
					<div style={c2}>VP9</div>
					<div style={c3}>{hardware(support.video.encoding?.vp9)}</div>
					<div style={c2}>VP8</div>
					<div style={c3}>{hardware(support.video.encoding?.vp8)}</div>
				</Show>
				<Show when={props.role !== "publish"}>
					<div style={c1}>Decoding</div>
					<div style={c2}>Opus</div>
					<div style={c3}>{binary(support.audio.decoding?.opus)}</div>
					<div style={c2}>AAC</div>
					<div style={c3}>{binary(support.audio.decoding?.aac)}</div>
					<div style={c2}>AV1</div>
					<div style={c3}>{hardware(support.video.decoding?.av1)}</div>
					<div style={c2}>H.265</div>
					<div style={c3}>{hardware(support.video.decoding?.h265)}</div>
					<div style={c2}>H.264</div>
					<div style={c3}>{hardware(support.video.decoding?.h264)}</div>
					<div style={c2}>VP9</div>
					<div style={c3}>{hardware(support.video.decoding?.vp9)}</div>
					<div style={c2}>VP8</div>
					<div style={c3}>{hardware(support.video.decoding?.vp8)}</div>
				</Show>
			</Show>
		</div>
	);
};
