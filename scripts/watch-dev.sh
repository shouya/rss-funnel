#!/bin/bash

trap "trap - SIGTERM && kill -- -$$" SIGINT SIGTERM EXIT

cargo watch -x build -s 'touch /tmp/.trigger' &
cargo watch -w /tmp/.trigger -s 'target/debug/rss-funnel -c ~/.config/rss-funnel-dev/funnel.yaml server' &

wait

