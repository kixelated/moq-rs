import { Source } from "./source"
import { Segment } from "./segment"
import { TimeRange } from "./util"

// An audio or video track that consists of multiple sequential segments.
//
// Instead of buffering, we want to drop video while audio plays uninterupted.
// Chrome actually plays up to 3s of audio without video before buffering when in low latency mode.
// Unforuntately, this does not recover correctly when there are gaps (pls fix).
// Our solution is to flush segments in decode order, buffering a single additional frame.
// We extend the duration of the buffered frame and flush it to cover any gaps.
export class Track {
	source: Source;
	segments: Segment[];

	constructor(source: Source) {
		this.source = source;
		this.segments = [];
	}

	add(segment: Segment) {
		// TODO don't add if the segment is out of date already
		this.segments.push(segment)

		// Sort by timestamp ascending
		// NOTE: The timestamp is in milliseconds, and we need to parse the media to get the accurate PTS/DTS.
		this.segments.sort((a: Segment, b: Segment): number => {
			return a.timestamp - b.timestamp
		})
	}

	buffered(): TimeRanges {
		let ranges: TimeRange[] = []

		const buffered = this.source.buffered() as TimeRanges
		for (let i = 0; i < buffered.length; i += 1) {
			// Convert the TimeRanges into an oject we can modify
			ranges.push({
				start: buffered.start(i),
				end: buffered.end(i)
			})
		}

		// Loop over segments and add in their ranges, merging if possible.
		for (let segment of this.segments) {
			const buffered = segment.buffered()
			if (!buffered) continue;

			if (ranges.length) {
				// Try to merge with an existing range
				const last = ranges[ranges.length-1];
				if (buffered.start < last.start) {
					// Network buffer is old; ignore it
					continue
				}

				// Extend the end of the last range instead of pushing
				if (buffered.start <= last.end && buffered.end > last.end) {
					last.end = buffered.end
					continue
				}
			}

			ranges.push(buffered)
		}

		// TODO typescript
		return {
			length: ranges.length,
			start: (x) => { return ranges[x].start },
			end: (x) => { return ranges[x].end },
		}
	}

	flush() {
		while (1) {
			if (!this.segments.length) break

			const first = this.segments[0]
			const done = first.flush()
			if (!done) break

			this.segments.shift()
		}
	}

	// Given the current playhead, determine if we should drop any segments
	// If playhead is undefined, it means we're buffering so skip to anything now.
	advance(playhead: number | undefined) {
		if (this.segments.length < 2) return

		while (this.segments.length > 1) {
			const current = this.segments[0];
			const next = this.segments[1];

			if (next.dts === undefined || next.timescale == undefined) {
				// No samples have been parsed for the next segment yet.
				break
			}

			if (current.dts === undefined) {
				// No samples have been parsed for the current segment yet.
				// We can't cover the gap by extending the sample so we have to seek.
				// TODO I don't think this can happen, but I guess we have to seek past the gap.
				break
			}

			if (playhead !== undefined) {
				// Check if the next segment has playable media now.
				// Otherwise give the current segment more time to catch up.
				if ((next.dts / next.timescale) > playhead) {
					return
				}
			}

			current.skipTo(next.dts || 0) // tell typescript that it's not undefined; we already checked
			current.finish()

			// TODO cancel the QUIC stream to save bandwidth

			this.segments.shift()
		}
	}
}
