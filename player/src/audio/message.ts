import * as MP4 from "../mp4"

export interface Config {
    ctx: AudioContext;
}

export interface Init {
    track: string;
    info: MP4.Info;
    raw: MP4.ArrayBufferOffset[];
}

export interface Segment {
    track: string;
    buffer: Uint8Array;     // unread buffered data
    reader: ReadableStream; // unread unbuffered data
}
