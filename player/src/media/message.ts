import * as MP4 from "../mp4"
import { RingInit } from "../media/ring"

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
    ring: RingInit;
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