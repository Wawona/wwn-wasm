# macOS may use Cranelift (no App Store JIT ban).
{
  lib,
  pkgs,
  common ? null,
  buildModule ? null,
  xcodeUtils,
  ...
}:

let
  wasmSrc = import ./wasm-src.nix { inherit pkgs; };
  cargoTarget = pkgs.stdenv.hostPlatform.rust.rustcTarget;
in
pkgs.rustPlatform.buildRustPackage {
  pname = "wawona-wasm";
  version = "0.1.0";
  src = "${wasmSrc}/source";
  cargoLock.lockFile = ./Cargo.lock;
  nativeBuildInputs = [ xcodeUtils.findXcodeScript ];
  CARGO_BUILD_TARGET = cargoTarget;
  doCheck = false;
  cargoBuildFlags = [ "--no-default-features" "--features" "cranelift-native" ];

  preConfigure = ''
    MACOS_SDK=$(xcrun --sdk macosx --show-sdk-path 2>/dev/null || true)
    if [ ! -d "$MACOS_SDK" ]; then
      MACOS_SDK="/Library/Developer/CommandLineTools/SDKs/MacOSX.sdk"
    fi
    export SDKROOT="$MACOS_SDK"
    export MACOSX_DEPLOYMENT_TARGET="26.0"
    export RUSTFLAGS="-A warnings $RUSTFLAGS"
  '';

  postInstall = ''
    mkdir -p $out/lib $out/include
    found=$(find target -name libwawona_wasm.a 2>/dev/null | head -1)
    if [ -n "$found" ]; then
      cp "$found" $out/lib/libwawona_wasm.a
    else
      echo "ERROR: libwawona_wasm.a not found" >&2
      exit 1
    fi
    mkdir -p $out/bin
    bin=$(find target -name wasm -type f -perm -111 2>/dev/null | head -1)
    [ -n "$bin" ] && cp "$bin" $out/bin/wasm
    cp ${wasmSrc}/source/include/wawona_wasm.h $out/include/ 2>/dev/null || true
  '';
}
