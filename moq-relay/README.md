# moq-relay

**moq-relay** is a server that forwards subscriptions from publishers to subscribers, caching and deduplicating along the way.
It's designed to be run in a datacenter, relaying media across multiple hops to deduplicate and improve QoS.

Required arguments:

-   `--bind <ADDR>`: Listen on this address, default: `[::]:4443`
-   `--tls-cert <CERT>`: Use the certificate file at this path
-   `--tls-key <KEY>` Use the private key at this path

This listens for WebTransport connections on `UDP https://localhost:4443` by default.
You need a client to connect to that address, to both publish and consume media.

## Clustering
In order to scale MoQ, you will eventually need to run multiple moq-relay instances potentially in different regions.
This is called *clustering*, where the goal is that a user connects to the closest relay and they magically form a mesh behind the scenes.

**moq-relay** uses a simple clustering scheme using MoqTransfork itself.
This is both dog-fooding and a surprisingly ueeful way to distribute live metadata at scale.

We currently use a single "root" node that is used to discover members of the cluster and what broadcasts they offer.
This is a normal moq-relay instance, potentially serving public traffic, unaware of the fact that it's in charge of other relays.

The other moq-relay instances accept internet traffic and consult the root for routing.
They can then advertise their internal ip/hostname to other instances when publishing a broadcast.

Cluster arguments:

-   `--cluster-root <HOST>`: The hostname/ip of the root node. If missing, this node is a root.
-   `--cluster-node <HOST>`: The hostname/ip of this instance. There needs to be a corresponding valid TLS certificate, potentially self-signed. If missing, published broadcasts will only be available on this specific relay.

## Authentication 
There is currently no authentication.
All broadcasts are public and discoverable.

However, track names are *not* public.
An application could make them unguessable in order to implement private broadcasts.

If security/privacy is a concern, you should encrypt all application payloads anyway (ex. via MLS).
moq-relay will **only** use the limited header information surfaced in the MoqTransfork layer.
