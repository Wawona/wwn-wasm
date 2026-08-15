#!/bin/sh
set -eu
cd "$(dirname "$0")"
# Requires Swift 6.3+ wasm SDK: swift sdk install <artifactbundle>
mkdir -p dist
swift build --swift-sdk swift-6.3.2-RELEASE_wasm -c release
cp .build/release/wasm-demo-swift.wasm dist/wasm-demo-swift.wasm
echo "-> dist/wasm-demo-swift.wasm"
