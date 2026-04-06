#!/bin/bash

set -e

SCRCPY_VERSION="3.3.4"

if [ ! -d "target/debug" ]; then
    mkdir -p target/debug
fi
if [ ! -d "target/release" ]; then
    mkdir -p target/release
fi

if [ ! -f "target/scrcpy-${SCRCPY_VERSION}.tar.gz" ]; then
    echo "Downloading scrcpy version ${SCRCPY_VERSION}..."
    wget -O target/scrcpy-${SCRCPY_VERSION}.tar.gz https://github.com/Genymobile/scrcpy/releases/download/v${SCRCPY_VERSION}/scrcpy-linux-x86_64-v${SCRCPY_VERSION}.tar.gz
fi

tar -xzf target/scrcpy-${SCRCPY_VERSION}.tar.gz -C target/debug --strip-components=1
tar -xzf target/scrcpy-${SCRCPY_VERSION}.tar.gz -C target/release --strip-components=1
