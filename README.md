# Warp
Segmented live media delivery protocol utilizing QUIC streams. See the [Warp draft](https://datatracker.ietf.org/doc/draft-lcurley-warp/).

Warp works by delivering each audio and video segment as a separate QUIC stream. These streams are assigned a priority such that old video will arrive last and can be dropped. This avoids buffering in many cases, offering the viewer a potentially better experience.

This demo includes a button that sends a custom message to throttle the network. This is not a realistic network simulation; you should evaluate Warp on real networks.

## Browser Support
This demo currently only works on Chrome for two reasons:

1. WebTransport support.
2. [Media underflow behavior](https://github.com/whatwg/html/issues/6359).

The ability to skip video abuses the fact that Chrome can play audio without video for up to 3 seconds (hardcoded!) when using MSE. It is possible to use something like WebCodecs instead... but that's still Chrome only at the moment.

# Setup
## Requirements
* Go
* ffmpeg
* openssl
* Chrome Canary

## Media
This demo simulates a live stream by reading a file from disk and sleeping based on media timestamps. Obviously you should hook this up to a real live stream to do anything useful.

Download your favorite media file:
```
wget http://commondatastorage.googleapis.com/gtv-videos-bucket/sample/BigBuckBunny.mp4 -O media/combined.mp4
```

Use ffmpeg to create a LL-DASH playlist. This creates a segment every 2s and MP4 fragment every 50ms.
```
ffmpeg -i media/combined.mp4 -f dash -use_timeline 0 -r:v 24 -g:v 48 -keyint_min:v 48 -sc_threshold:v 0 -tune zerolatency -streaming 1 -ldash 1 -seg_duration 2 -frag_duration 0.01 -frag_type duration media/fragmented.mpd
```

You can increase the `frag_duration` (microseconds) to slightly reduce the file size in exchange for higher latency.

## TLS
Unfortunately, QUIC mandates TLS and makes local development difficult.

If you have a valid certificate you can use it instead of self-signing. The go binaries take a `-cert` and `-key` argument. Skip the remaining steps in this section and use your hostname instead of `localhost.warp.demo`.

### Self-Sign
Generate a self-signed certificate for local testing:
```
./cert/generate
```

This creates `cert/localhost.warp.demo.crt` and `cert/localhost.warp.demo.key`.

### Origin
To have the browser accept our self-signed certificate, you'll need to add an entry to `/etc/hosts`.

```
echo '127.0.0.1 localhost.warp.demo' | sudo tee -a /etc/hosts
```

### Chrome
Now we need to make Chrome accept these certificates, which normally would involve trusting a root CA but this was not working with WebTransport when I last tried.

Instead, we need to run a *fresh instance* of Chrome, instructing it to allow our self-signed certificate. This command will not work if Chrome is already running, so it's easier to use Chrome Canary instead. This command also needs to be executed in the project root because it invokes `./cert/fingerprint`.

Launch a new instance of Chrome Canary:
```
/Applications/Google\ Chrome\ Canary.app/Contents/MacOS/Google\ Chrome\ Canary --origin-to-force-quic-on="localhost.warp.demo:4443" --ignore-certificate-errors-spki-list="`./cert/fingerprint`" https://localhost.warp.demo:4444
```

Note that this will open our web server on `localhost.warp.demo:4444`, which is started in the next section.

## Server
The Warp server defaults to listening on UDP 4443. It supports HTTP/3 and WebTransport, pushing media over WebTransport streams once a connection has been established. A more refined implementation would load content based on the WebTransport URL or some other messaging scheme.

```
cd server
go run ./warp-server
```

## Web
The web assets need to be hosted with a HTTPS server. If you're using a self-signed certificate, you will need to ignore the security warning in Chrome (Advanced -> proceed to localhost.warp.demo). This can be avoided by adding your certificate to the root CA but I'm too lazy to do that.

```
cd client
yarn serve
```

These can be accessed on `https://localhost.warp.demo:4444` by default.
