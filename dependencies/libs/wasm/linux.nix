{
  lib,
  pkgs,
  ...
}:

let
  wasmSrc = import ./wasm-src.nix { inherit pkgs; };
in
pkgs.rustPlatform.buildRustPackage {
  pname = "wawona-wasm";
  version = "0.1.0";
  src = "${wasmSrc}/source";
  cargoLock.lockFile = ./Cargo.lock;
  doCheck = false;
  cargoBuildFlags = [ "--no-default-features" "--features" "cranelift-native" ];
  postInstall = ''
    mkdir -p $out/lib $out/include
    found=$(find target -name libwawona_wasm.a 2>/dev/null | head -1)
    [ -n "$found" ] && cp "$found" $out/lib/libwawona_wasm.a
    mkdir -p $out/bin
    bin=$(find target -name wasm -type f -perm -111 2>/dev/null | head -1)
    [ -n "$bin" ] && cp "$bin" $out/bin/wasm
    cp ${wasmSrc}/source/include/wawona_wasm.h $out/include/ 2>/dev/null || true
  '';
}
