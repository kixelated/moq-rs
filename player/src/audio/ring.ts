// Ring buffer with audio samples.

enum STATE {
    READ_INDEX = 0, // Index of the current read position (mod capacity)
    WRITE_INDEX,    // Index of the current write position (mod capacity)
    LENGTH          // Clever way of saving the total number of enums values.
}

export class Ring {
    state: Int32Array;
    channels: Float32Array[];
    capacity: number;

    constructor(init: RingInit) {
        this.state = new Int32Array(init.state)

        this.channels = []
        for (let channel of init.channels) {
            this.channels.push(new Float32Array(channel))
        }

        this.capacity = init.capacity
    }

    // Add the samples for single audio frame
    write(frame: AudioData): boolean {
        let count = frame.numberOfFrames;

        let readIndex = Atomics.load(this.state, STATE.READ_INDEX)
        let writeIndex = Atomics.load(this.state, STATE.WRITE_INDEX)
        let writeIndexNew = writeIndex + count;

        // There's not enough space in the ring buffer
        if (writeIndexNew - readIndex > this.capacity) {
            return false
        }

        let startIndex = writeIndex % this.capacity;
        let endIndex = writeIndexNew % this.capacity;

        // Loop over each channel
        for (let i = 0; i < this.channels.length; i += 1) {
            const channel = this.channels[i]

            if (startIndex < endIndex) {
                // One continuous range to copy.
                const full = channel.subarray(startIndex, endIndex)

                frame.copyTo(full, {
                    planeIndex: i,
                    frameCount: count,
                })
            } else {
                const first = channel.subarray(startIndex)
                const second = channel.subarray(0, endIndex)

                frame.copyTo(first, {
                    planeIndex: i,
                    frameCount: first.length,
                })

               //For some reason this breaks audio... and this is my temporary fix
               //console.log("frame offset", first.length , "frame count", second.length) to test
               if (first.length < second.length) {
                frame.copyTo(second, {
                    planeIndex: i,
                    frameOffset: first.length,
                    frameCount: second.length,
                })
              }
            }
        }

        Atomics.store(this.state, STATE.WRITE_INDEX, writeIndexNew)

        return true
    }

    read(dst: Float32Array[]) {
        let readIndex = Atomics.load(this.state, STATE.READ_INDEX)
        let writeIndex = Atomics.load(this.state, STATE.WRITE_INDEX)
        if (readIndex >= writeIndex) {
            // nothing to read
            return
        }

        let readIndexNew = readIndex + dst[0].length
        if (readIndexNew > writeIndex) {
            // Partial read
            readIndexNew = writeIndex
        }

        let startIndex = readIndex % this.capacity;
        let endIndex = readIndexNew % this.capacity;

        // Loop over each channel
        for (let i = 0; i < dst.length; i += 1) {
            if (i >= this.channels.length) {
                // ignore excess channels
            }

            const input = this.channels[i]
            const output = dst[i]

            if (startIndex < endIndex) {
                const full = input.subarray(startIndex, endIndex)
                output.set(full)
            } else {
                const first = input.subarray(startIndex)
                const second = input.subarray(0, endIndex)

                output.set(first)
                output.set(second, first.length)
            }
        }

        Atomics.store(this.state, STATE.READ_INDEX, readIndexNew)
    }

    // TODO not thread safe
    clear() {
        const writeIndex = Atomics.load(this.state, STATE.WRITE_INDEX)
        Atomics.store(this.state, STATE.READ_INDEX, writeIndex)
    }
}

// No prototype to make this easier to send via postMessage
export class RingInit {
    state: SharedArrayBuffer;

    channels: SharedArrayBuffer[];
    capacity: number;

    constructor(channels: number, capacity: number) {
        // Store the current state in a separate ring buffer.
        this.state = new SharedArrayBuffer(STATE.LENGTH * Int32Array.BYTES_PER_ELEMENT)

        // Create a buffer for each audio channel
        this.channels = []
        for (let i = 0; i < channels; i += 1) {
            const buffer = new SharedArrayBuffer(capacity * Float32Array.BYTES_PER_ELEMENT)
            this.channels.push(buffer)
        }

        this.capacity = capacity
    }
}
