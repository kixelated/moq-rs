// This file was generated by [ts-rs](https://github.com/Aleph-Alpha/ts-rs). Do not edit this file manually.

export type PublishCommand = { "Connect": string | null } | { "AudioInit": { sample_rate: number, channel_count: number, } } | { "AudioFrame": AudioData } | "AudioClose" | { "VideoInit": { width: number, height: number, } } | { "VideoFrame": VideoFrame } | "VideoClose";
