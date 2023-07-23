#!/usr/bin/env sh

mkdir cert
echo "$MOQ_CRT" | base64 -d > cert/moq-demo.crt
echo "$MOQ_KEY" | base64 -d > cert/moq-demo.key

# while true; do
#     echo "."
#     sleep 5
# done

RUST_LOG=info ./moq-demo --cert cert/moq-demo.crt --key cert/moq-demo.key
