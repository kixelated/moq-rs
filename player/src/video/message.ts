import * as MP4 from "../mp4"

export interface Config {
    canvas: OffscreenCanvas;
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