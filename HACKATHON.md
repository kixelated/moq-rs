# Hackathon

IETF Prague 118

## MoqTransport

Reference libraries are available at [moq-rs](https://github.com/kixelated/moq-rs) and [moq-js](https://github.com/kixelated/moq-js). The Rust library is [well documented](https://docs.rs/moq-transport/latest/moq_transport/) but the web library, not so much.

**TODO** Update both to draft-01.
**TODO** Switch any remaining forks over to extensions. ex: track_id in SUBSCRIBE

The stream mapping right now is quite rigid: `stream == group == object`.

**TODO** Support multiple objects per group. They MUST NOT use different priorities, different tracks, or out-of-order sequences.

The API and cache aren't designed to send/receive arbitrary objects over arbitrary streams as specified in the draft. I don't think it should, and it wouldn't be possible to implement in time for the hackathon anyway.

**TODO** Make an extension to require this stream mapping?

## Generic Relay

I'm hosting a simple CDN at: `relay.quic.video`

The traffic is sharded based on the WebTransport path to avoid namespace collisions. Think of it like a customer ID, although it's completely unauthenticated for now. Use your username or whatever string you want: `CONNECT https://relay.quic.video/alan`.

**TODO** Currently, it performs an implicit `ANNOUNCE ""` when `role=publisher`. This means there can only be a single publisher per shard and `role=both` is not supported. I should have explicit `ANNOUNCE` messages supported before the hackathon to remove this limitation.

**TODO** I don't know if I will have subscribe hints fully working in time. They will be parsed but might be ignored.

## CMAF Media

You can [publish](https://quic.video/publish) and [watch](https://quic.video/watch) broadcasts.
You can also use `moq-pub` to publish your own media if you would like.

I'm currently using a JSON catalog similar to the proposed [Warp catalog](https://datatracker.ietf.org/doc/draft-wilaw-moq-catalogformat/).

**TODO** update to the proposed format.

If you want to fetch from the relay directly, the name of the broadcast is path. For example, `https://quic.video/watch/bbb` can be accessed at `relay.quic.video/bbb`. The namespace is empty and the catalog track is `.catalog`.

Each of the tracks uses a single object per groups. Video groups are per GoP, while audio groups are per frame. There's also an init track containing information required to initialize the decoder.

**TODO** Base64 encode the init track in the catalog.

**TODO** Add a flag for publishing LoC media? It shouldn't be difficult.

## Clock

**TODO** Host a clock demo that sends a group per second:

```
GROUP: YYYY-MM-DD HH:MM
OBJECT: SS
```
