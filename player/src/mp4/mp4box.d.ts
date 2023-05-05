// https://github.com/gpac/mp4box.js/issues/233

declare module "mp4box" {
    export interface MP4MediaTrack {
        id: number;
        created: Date;
        modified: Date;
        movie_duration: number;
        layer: number;
        alternate_group: number;
        volume: number;
        track_width: number;
        track_height: number;
        timescale: number;
        duration: number;
        bitrate: number;
        codec: string;
        language: string;
        nb_samples: number;
    }

    export interface MP4VideoData {
        width: number;
        height: number;
    }

    export interface MP4VideoTrack extends MP4MediaTrack {
        video: MP4VideoData;
    }

    export interface MP4AudioData {
        sample_rate: number;
        channel_count: number;
        sample_size: number;
    }

    export interface MP4AudioTrack extends MP4MediaTrack {
        audio: MP4AudioData;
    }

    export type MP4Track = MP4VideoTrack | MP4AudioTrack;

    export interface MP4Info {
        duration: number;
        timescale: number;
        fragment_duration: number;
        isFragmented: boolean;
        isProgressive: boolean;
        hasIOD: boolean;
        brands: string[];
        created: Date;
        modified: Date;
        tracks: MP4Track[];
        mime: string;
        audioTracks: MP4AudioTrack[];
        videoTracks: MP4VideoTrack[];
    }

    export type MP4ArrayBuffer = ArrayBuffer & {fileStart: number};

    export interface MP4File {
        onMoovStart?: () => void;
        onReady?: (info: MP4Info) => void;
        onError?: (e: string) => void;
        onSamples?: (id: number, user: any, samples: Sample[]) => void;

        appendBuffer(data: MP4ArrayBuffer): number;
        start(): void;
        stop(): void;
        flush(): void;

        setExtractionOptions(id: number, user: any, options: ExtractionOptions): void;
    }

    export function createFile(): MP4File;

    export interface Sample {
        number: number;
        track_id: number;
        timescale: number;
        description_index: number;
        description: any;
        data: ArrayBuffer;
        size: number;
        alreadyRead: number;
        duration: number;
        cts: number;
        dts: number;
        is_sync: boolean;
        is_leading: number;
        depends_on: number;
        is_depended_on: number;
        has_redundancy: number;
        degration_priority: number;
        offset: number;
        subsamples: any;
    }

    export interface ExtractionOptions {
        nbSamples: number;
    }

    const BIG_ENDIAN: boolean;
    const LITTLE_ENDIAN: boolean;

    export class DataStream {
        constructor(buffer: ArrayBuffer, byteOffset?: number, littleEndian?: boolean);
        getPosition(): number;

        get byteLength(): number;
        get buffer(): ArrayBuffer;
        set buffer(v: ArrayBuffer);
        get byteOffset(): number;
        set byteOffset(v: number);
        get dataView(): DataView;
        set dataView(v: DataView);

        seek(pos: number): void;
        isEof(): boolean;

        mapUint8Array(length: number): Uint8Array;
        readInt32Array(length: number, littleEndian: boolean): Int32Array;
        readInt16Array(length: number, littleEndian: boolean): Int16Array;
        readInt8(length: number): Int8Array;
        readUint32Array(length: number, littleEndian: boolean): Uint32Array;
        readUint16Array(length: number, littleEndian: boolean): Uint16Array;
        readUint8(length: number): Uint8Array;
        readFloat64Array(length: number, littleEndian: boolean): Float64Array;
        readFloat32Array(length: number, littleEndian: boolean): Float32Array;

        readInt32(littleEndian: boolean): number;
        readInt16(littleEndian: boolean): number;
        readInt8(): number;
        readUint32(littleEndian: boolean): number;
        readUint16(littleEndian: boolean): number;
        readUint8(): number;
        readFloat32(littleEndian: boolean): number;
        readFloat64(littleEndian: boolean): number;

        endianness: boolean;

        memcpy(dst: ArrayBufferLike, dstOffset: number, src: ArrayBufferLike, srcOffset: number, byteLength: number): void;

        // TODO I got bored porting the remaining functions
    }

    export { };
}