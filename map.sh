#!/bin/bash

set -e -u

cargo run --release -- --cache-dir cache from-usb-notecard --usb-notecard "$1" --color '#0f0' --max-width 2048 --max-height 2048 --output-file "${1//.txt}.png" --missing-map-tile-color '#000' --missing-region-color '#000'
