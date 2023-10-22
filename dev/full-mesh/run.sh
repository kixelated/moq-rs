#!/bin/bash
set -euo pipefail

cleanup() {
	jobs -p | xargs kill
}

trap cleanup EXIT

echo "Starting moq-api"
PORT=4440 ./dev/api &

# Running relays without parameters results in a full mesh
echo "Starting relays"
for ((i = 1; i <= 5; i++)); do
	# Using relay-0 as it has NODE specified to self
	export PORT="$((4440 + i))"
	export API="http://localhost:4440"
	export NODE="https://localhost:${PORT}"
	./dev/relay &
done

while true; do
	sleep 100
done
