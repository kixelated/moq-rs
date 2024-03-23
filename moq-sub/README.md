# moq-sub

A command line tool for subscribing to media via Media over QUIC (MoQ).

Takes an URL to MoQ relay with a broadcast name in the path part of the URL. It will connect to the relay, subscribe to
the broadcast, and dump the media segments of the first video and first audio track to STDOUT.

```
moq-sub https://localhost:4443/dev | ffplay -
```
