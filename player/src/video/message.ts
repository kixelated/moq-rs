import Reader from "../stream/reader";

export interface Config {
    canvas: OffscreenCanvas;
}

export interface Init {
    track: string;
    stream: Reader;
}

export interface Segment {
    track: string;
    stream: Reader;
}