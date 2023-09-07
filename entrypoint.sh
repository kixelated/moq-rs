#!/usr/bin/env sh

mkdir cert
# Nothing to see here...
echo "$MOQ_CRT" | base64 -d > cert/moq-demo.crt
echo "$MOQ_KEY" | base64 -d > cert/moq-demo.key

RUST_LOG=info ./moq-quinn --cert cert/moq-demo.crt --key cert/moq-demo.key
