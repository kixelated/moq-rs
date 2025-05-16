# Usage
WebComponents cause some issues with tree-shaking.
If you're going to use the `<moq-*>` tags in your HTML, make sure your bundler is properly configured to not tree-shake them out.

## UI
All of the components come with an optional UI element.
This is not required, as you can build your own using the events/attributes, but it is a lot easier to get started with.

Unfortunately, we're using Shoelace which requires some extra work to install.
The icons [need to be hosted](https://shoelace.style/getting-started/installation#setting-the-base-path) as part of your website.
See the [webpack.config](../webpack.config.mjs) for an example of how to do this.

## Watch
Watch a single broadcast.

```html
<script type="module">
	import "@kixelated/moq/watch"

	// Optional UI element which wraps the <moq-watch> tag.
	// import "@kixelated/moq/watch/ui"
</script>

<!-- <moq-watch-ui> -->
	<moq-watch url="https://relay.quic.video/demo/bbb"></moq-watch>
<!-- </moq-watch-ui> -->
```

Notable attributes:
- `url`: The URL of the Karp broadcast to watch. A `null` value is useful for preloading the Worker+WASM. (default: `null`)
- `paused`: If non-null, the video will be paused. (default: `null`)
- `latency`: The target latency in milliseconds. (default: `0`)


## Publish
Publish a single broadcast.

```html
<script type="module">
	import "@kixelated/moq/publish"

	// Optional UI element which wraps the <moq-publish> tag.
	// import "@kixelated/moq/publish/ui"
</script>

<!-- <moq-publish-ui> -->
	<moq-publish url="https://relay.quic.video/demo/bbb"></moq-publish>
<!-- </moq-publish-ui> -->
```

Notable attributes:
- `url`: The URL of the Karp broadcast to watch. A `null` value is useful for preloading the WASM. (default: `null`)
- `device`: The device to capture: `"camera" | "screen" | "none" | null`. `"none"` will publish an empty broadcast, while `null` will not publish anything. (default: `null`)
- `preview`: If non-null, the captured video will be rendered. (default: `null`)


## Meet
Watch multiple broadcasts (in a grid) that match a prefix.

```html
<script type="module">
	import "@kixelated/moq/meet"

	// Optional UI element which wraps the <moq-meet> tag.
	// import "@kixelated/moq/meet/ui"
</script>

<!-- <moq-meet-ui> -->
	<moq-meet url="https://relay.quic.video/demo"></moq-meet>
<!-- </moq-meet-ui> -->
```

Notable attributes:
- `url`: The URL of the meeting to watch. Any broadcast that starts with this prefix will be included. (default: `null`)

## Video
An element that implements a subset of the [HTMLVideoElement](https://developer.mozilla.org/en-US/docs/Web/API/HTMLVideoElement) API.
Useful with [Media Chrome](https://www.media-chrome.org/) or existing video players.

```html
<script type="module">
	import "@kixelated/moq/video"
</script>

<moq-video src="https://relay.quic.video/demo/bbb" autoplay></moq-video>
```

This element is maintained on a best-effort.
If you want full functionality, use the `<moq-watch>` element.


# Development

The package is a gross frankenstein of Rust+Typescript.
To run the demo page:

```sh
just web
```

If you're importing the `@kixelated/moq` package within your application, you can also test the package locally by linking.
This will create a symlink in `node_modules` which can cause some issues, but should work.

```sh
# Builds and runs `npm link`
just link

# In your other package
npm link @kixelated/moq
```
