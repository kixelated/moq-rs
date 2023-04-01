import Ring from "./ring"

// Sent to the worklet to share ring buffers.
export interface Config {
    channels: Ring[];
}

// Reply from the worklet indicating when the configuration was suscessful.
export interface ConfigReply {
    error?: Error;
}