// Ring buffer with audio samples.

enum STATE {
    START = 0, // First timestamp in the ring buffer.
    END,       // Last timestamp in the ring buffer.
    LENGTH     // Clever way of saving the total number of enums values.
}

export class Ring {
    state: RingState;

    constructor(state: RingState) {
        this.state = state
    }

    // Add the samples for single audio frame with the given channel
    emit(timestamp: number, frame: AudioData, channel: number) {
        let start = timestamp;

        // The number of samples to skip at the start.
        let offset = this.start - timestamp;
        if (offset > 0) {
            console.warn("dropping old samples", offset)
            start += offset;
        } else {
            offset = 0
        }

        let count = frame.numberOfFrames - offset;
        if (count <= 0) {
            frame.close()

            // Skip the entire frame
            return
        }

        let end = timestamp + count;

        if (end >= start + this.state.capacity) {
            // The renderer has to buffer frames; we have a fixed capacity.
            // TODO maybe it's better to buffer here instead.
            throw new Error("exceeded capacity")
        }

        const startIndex = start % this.state.capacity;
        const endIndex = end % this.state.capacity;

        if (startIndex < endIndex) {
            // One continuous range to copy.
            const full = new Float32Array(this.state.buffer, 4*startIndex, endIndex-startIndex)

            frame.copyTo(full, {
                planeIndex: channel,
                frameOffset: offset,
                frameCount: count,
            })
        } else {
            // Wrapped around the ring buffer, so we have to copy twice.
            const wrap = this.state.capacity - startIndex;

            const first = new Float32Array(this.state.buffer, 4*startIndex, this.state.capacity - startIndex)
            const second = new Float32Array(this.state.buffer, 0, endIndex)

            frame.copyTo(first, {
                planeIndex: channel,
                frameOffset: offset,
                frameCount: wrap,
            })

            frame.copyTo(second, {
                planeIndex: channel,
                frameOffset: offset + wrap,
                frameCount: endIndex,
            })
        }

        if (this.end < end) {
            this.end = end
        }
    }

    peek(count: number): Float32Array[] {
        const start = this.start

        let end = this.end
        if (end > start + count) {
            end = start + count
        }

        const startIndex = start % this.state.capacity;
        const endIndex = end % this.state.capacity;

        if (startIndex < endIndex) {
            const full = new Float32Array(this.state.buffer, 4*startIndex, endIndex - startIndex)
            return [ full ]
        } else {
            const first = new Float32Array(this.state.buffer, 4*startIndex, this.state.capacity - startIndex)
            const second = new Float32Array(this.state.buffer, 0, endIndex)
            return [ first, second ]
        }
    }

    advance(count: number) {
        this.start += count
    }

    set start(start: number) {
        Atomics.store(this.state.stateView, STATE.START, start);
    }

    get start(): number {
        return Atomics.load(this.state.stateView, STATE.START);
    }

    set end(end: number) {
        Atomics.store(this.state.stateView, STATE.END, end);
    }

    get end(): number {
        return Atomics.load(this.state.stateView, STATE.END);
    }
}

// No prototype to make this easier to send via postMessage
export class RingState {
    state: SharedArrayBuffer;
    stateView: Int32Array;

    buffer: SharedArrayBuffer;

    capacity: number;

    constructor(capacity: number) {
        // Store this many samples in a ring buffer.
        this.buffer = new SharedArrayBuffer(capacity * Float32Array.BYTES_PER_ELEMENT)
        this.capacity = capacity

        // Store the current state in a separate ring buffer.
        this.state = new SharedArrayBuffer(STATE.LENGTH * Int32Array.BYTES_PER_ELEMENT)
        this.stateView = new Int32Array(this.state)
    }
}