# moq-relay

A server that connects publishing clients to subscribing clients.
All subscriptions are deduplicated and cached, so that a single publisher can serve many subscribers.

## Usage

The publisher must choose a unique name for their broadcast, sent as the WebTransport path when connecting to the server.
We currently do a dumb string comparison, so capatilization matters as do slashes.

For example: `CONNECT https://relay.quic.video/BigBuckBunny`

The MoqTransport handshake includes a `role` parameter, which must be `publisher` or `subscriber`.
The specification allows a `both` role but you'll get an error.

You can have one publisher and any number of subscribers connected to the same path.
If the publisher disconnects, then all subscribers receive an error and will not get updates, even if a new publisher reuses the path.
