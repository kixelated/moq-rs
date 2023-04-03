import * as MP4 from "../mp4"
import { RingState } from "./ring"

export interface Config {
    sampleRate: number;
    channels: RingState[];
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