#!/usr/bin/env sh

mkdir cert
# Nothing to see here...
echo "$MOQ_CRT" | base64 -d > dev/moq-demo.crt
echo "$MOQ_KEY" | base64 -d > dev/moq-demo.key

RUST_LOG=info /usr/local/cargo/bin/moq-relay --tls-cert dev/moq-demo.crt --tls-key dev/moq-demo.key
