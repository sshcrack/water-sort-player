#!/bin/bash
stdbuf -oL ./target/debug/scrcpy --stay-awake --no-audio --mouse=disabled --keyboard=disabled --gamepad=disabled --max-size=800 --max-fps=15 --video-bit-rate=2M --video-codec=h264 --no-clipboard-autosync --window-title=AutoPlayer --no-video-playback --v4l2-sink=/dev/video10
