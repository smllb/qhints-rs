#!/usr/bin/env bash
exec /home/yogi/qhints-rs/target/release/qhints-rs "$@" 2>&1 | logger -t qhints-rs
