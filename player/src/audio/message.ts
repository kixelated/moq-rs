import * as MP4 from "../mp4"
import { RingInit } from "./ring"

export interface Config {
    sampleRate: number;
    ring: RingInit;
}

export interface Init {
    track: string;
    info: MP4.Info;
    raw: MP4.ArrayBuffer[];
}

export interface Segment {
    track: string;
    buffer: Uint8Array;     // unread buffered data
    reader: ReadableStream; // unread unbuffered data
}

// Audio tells video when the given timestamp should be rendered.
export interface Sync {
    origin: number;
    clock: DOMHighResTimeStamp;
    timestamp: number;
}

export interface Play {
    timestamp?: number;
}