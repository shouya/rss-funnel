#!/bin/bash

trap "trap - SIGTERM && kill -- -$$" SIGINT SIGTERM EXIT

cargo watch -x build -s 'touch /tmp/.trigger' &
cargo watch -w /tmp/.trigger -x 'run -- -c ~/.config/rss-funnel-dev/funnel.yaml server' &

wait

