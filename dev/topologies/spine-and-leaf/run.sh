#!/bin/bash
set -euo pipefail

cleanup() {
	jobs -p | xargs kill
}

trap cleanup EXIT

echo "Starting moq-api"
PORT=4440 ./dev/api &

echo "Starting spine"
for ((i = 1; i <= 2; i++)); do
	export PORT="$((4440 + i))"
	export API="http://localhost:4440"
	export NODE="https://localhost:${PORT}"
	./dev/relay &
done

echo "Starting leaf"
for ((i = 3; i <= 5; i++)); do
	export PORT="$((4440 + i))"
	export API="http://localhost:4440"
	export NODE="https://localhost:${PORT}"
	export ARGS="--next-relays https://localhost:4441 --next-relays https://localhost:4442"
	./dev/relay &
done

while true; do
	sleep 100
done
