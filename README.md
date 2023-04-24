# WARP as a MOQ Protocol

Warp is a live media transport protocol over QUIC.  Media is split into objects based on the underlying media encoding and transmitted independently over QUIC streams.  QUIC streams are prioritized based on the delivery order, allowing less important objects to be starved or dropped during congestion.

See the [Warp draft](https://datatracker.ietf.org/doc/draft-lcurley-warp/).

# MOQ Testbed

This demo has been forked off the original [WARP demo application code](https://github.com/kixelated/warp).

You can find a working demo [here](https://moq.streaming.university).

There are numerous additions as follows:

- Server-to-client informational messages: Added Common Media Server Data (CMSD, CTA-5006)
keys: Availability Time (at) and Estimated Throughput (ETP).
- Client-to-server control messages
- Passive bandwidth measurements: Sliding Window Moving Average with Threshold (SWMAth) and I-Frame Average (IFA).
- Active bandwidth measurements
- Enhanced user interface

More information and test results have been published in the following paper:

Z. Gurel, T. E. Civelek, A. Bodur, S. Bilgin, D. Yeniceri, and A. C. Begen. Media
over QUIC: Initial testing, findings and results. In ACM MMSys, 2023.