import * as Ring from "./ring"

export interface Config {
    audio: AudioConfig;
    video: VideoConfig;
}

export interface VideoConfig {
    canvas: OffscreenCanvas;
}

export interface AudioConfig {
    // audio stuff
    sampleRate: number;
    ring: Ring.Buffer;
}

export interface Init {
    buffer: Uint8Array;     // unread buffered data
    reader: ReadableStream; // unread unbuffered data
}

export interface Segment {
    buffer: Uint8Array;     // unread buffered data
    reader: ReadableStream; // unread unbuffered data
}

export interface Play {
    timestamp?: number;
}