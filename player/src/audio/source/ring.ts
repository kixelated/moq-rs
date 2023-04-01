// Ring buffer with audio samples.

// TODO typescript enums when I have internet access
const STATE = {
    START: 0,
    END: 1,
}

export default class Ring {
    state: SharedArrayBuffer;
    stateView: Int32Array;

    buffer: SharedArrayBuffer;
    capacity: number;

    constructor(samples: number) {
        this.state = new SharedArrayBuffer(Object.keys(STATE).length * Int32Array.BYTES_PER_ELEMENT)
        this.stateView = new Int32Array(this.state)

        this.setStart(0)
        this.setEnd(0)

        this.capacity = samples;

        // TODO better way to loop in modern Javascript?
        this.buffer = new SharedArrayBuffer(samples * Float32Array.BYTES_PER_ELEMENT)
    }

    setStart(start: number) {
        return Atomics.store(this.stateView, STATE.START, start);
    }

    getStart(): number {
        return Atomics.load(this.stateView, STATE.START);
    }

    setEnd(end: number) {
        return Atomics.store(this.stateView, STATE.START, end);
    }

    getEnd(): number {
        return Atomics.load(this.stateView, STATE.END);
    }

    set(frame: AudioFrame, channel: number) {
        let start = this.getStart()

        // The number of samples to skip at the start.
        let offset = start - frame.timestamp;
        if (offset > 0) {
            console.warn("dropping old samples", offset)
        } else {
            offset = 0
        }

        let count = frame.numberOfFrames - offset;
        if (count <= 0) {
            frame.close()

            // Skip the entire frame
            return
        }

        if (start + this.capacity < frame.timestamp + count) {
            // The renderer has to buffer frames; we have a fixed capacity.
            // TODO maybe it's better to buffer here instead.
            throw new Error("exceeded capacity")
        }

        let end = this.getEnd()

        const startIndex = start % this.capacity;
        const endIndex = end % this.capacity;

        if (startIndex < endIndex) {
            // One continuous range to copy.
            const full = new Float32Array(this.buffer, startIndex, endIndex-startIndex)

            frame.copyTo(full, {
                planeIndex: channel,
                frameOffset: offset,
                frameCount: count,
            })
        } else {
            // Wrapped around the ring buffer, so we have to copy twice.
            const wrap = this.capacity - startIndex;

            const first = new Float32Array(this.buffer, startIndex)
            const second = new Float32Array(this.buffer, 0, endIndex)

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

        // TODO insert silence when index > end
        if (frame.timestamp + count > end) {
            end = frame.timestamp + count
            this.setEnd(end)
        }
    }

    peek(count: number): Float32Array[] {
        const start = this.getStart()
        const end = this.getEnd()

        const startIndex = start % this.capacity;
        const endIndex = end % this.capacity;

        if (startIndex < endIndex) {
            const full = new Float32Array(this.buffer, startIndex, endIndex - startIndex)
            return [ full ]
        } else {
            const first = new Float32Array(this.buffer, startIndex)
            const second = new Float32Array(this.buffer, 0, endIndex)
            return [ first, second ]
        }
    }

    advance(count: number) {
        this.setStart(this.getStart() + count)
    }
}