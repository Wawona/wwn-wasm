# wawona-wasm for Apple mobile — Pulley interpreter only.
# Never enable the `cranelift-native` Cargo feature on this recipe (App Store 2.5.2).
# The `pulley` feature may use cranelift as a WASM→Pulley frontend; Config::target("pulley64")
# prevents native ARM emission.
{
  lib,
  pkgs,
  buildPackages ? pkgs.buildPackages,
  common ? null,
  buildModule ? null,
  simulator ? false,
  iosToolchain,
  ...
}:

let
  xcodeUtils = iosToolchain;
  isWatchOS = iosToolchain.isWatchOSToolchain or false;
  isTVOS = iosToolchain.isTVOSToolchain or false;
  isVisionOS = iosToolchain.isVisionOSToolchain or false;

  cargoTarget =
    if isWatchOS then
      (if simulator then "aarch64-apple-watchos-sim" else "aarch64-apple-watchos")
    else if isTVOS then
      (if simulator then "aarch64-apple-tvos-sim" else "aarch64-apple-tvos")
    else if isVisionOS then
      (if simulator then "aarch64-apple-visionos-sim" else "aarch64-apple-visionos")
    else if simulator then
      "aarch64-apple-ios-sim"
    else
      "aarch64-apple-ios";

  deploymentTargetEnv =
    if isWatchOS then ''
      export WATCHOS_DEPLOYMENT_TARGET="${iosToolchain.deploymentTarget}"
      unset IPHONEOS_DEPLOYMENT_TARGET
    ''
    else if isTVOS then ''
      export TVOS_DEPLOYMENT_TARGET="${iosToolchain.deploymentTarget}"
      unset IPHONEOS_DEPLOYMENT_TARGET
    ''
    else if isVisionOS then ''
      export XROS_DEPLOYMENT_TARGET="${iosToolchain.deploymentTarget}"
      unset IPHONEOS_DEPLOYMENT_TARGET
    ''
    else ''
      export IPHONEOS_DEPLOYMENT_TARGET="${iosToolchain.deploymentTarget}"
    '';

  wasmSrc = import ./wasm-src.nix { inherit pkgs; };
  rustToolchain = pkgs.rust-bin.stable.latest.default.override {
    targets = [ cargoTarget ];
  };
  myRustPlatform = pkgs.makeRustPlatform {
    cargo = rustToolchain;
    rustc = rustToolchain;
  };
in
# watchOS: size-gated off (same as coreutils). Produce an empty archive so
# recipes exist, but do not compile Wasmtime.
if isWatchOS then
  pkgs.runCommand "wawona-wasm-watchos-off" { } ''
    mkdir -p $out/lib $out/include
    echo "watchOS: wwn-wasm is size-gated off" > $out/README.txt
    # No libwawona_wasm.a — dispatch stays weak-NULL.
  ''
else
  myRustPlatform.buildRustPackage {
    pname = "wawona-wasm";
    version = "0.1.0";
    src = "${wasmSrc}/source";
    __noChroot = true;
    cargoLock.lockFile = ./Cargo.lock;
    CARGO_BUILD_TARGET = cargoTarget;
    doCheck = false;
    # Pulley only — CI verify script bans cranelift-native on this file.
    cargoBuildFlags = [ "--no-default-features" "--features" "pulley" ];

    preConfigure = ''
      ${xcodeUtils.mkIOSBuildEnv { inherit simulator; }}
      export NIX_CFLAGS_COMPILE=""
      export NIX_CXXFLAGS_COMPILE=""
      export NIX_LDFLAGS=""
      ${deploymentTargetEnv}
      export RUSTFLAGS="-A warnings -C linker=$XCODE_CLANG -C link-arg=-isysroot -C link-arg=$SDKROOT -C link-arg=$APPLE_DEPLOYMENT_FLAG $RUSTFLAGS"
      target_underscore=$(echo "${cargoTarget}" | tr '-' '_')
      export "CC_''${target_underscore}"="$XCODE_CLANG"
      export "CXX_''${target_underscore}"="$XCODE_CLANGXX"
      export "AR_''${target_underscore}"="ar"
      export "CARGO_TARGET_''${target_underscore^^}_LINKER"="$XCODE_CLANG"
      export MACOS_SDK=$(xcrun --sdk macosx --show-sdk-path 2>/dev/null || true)
      export HOST_CC="/usr/bin/clang"
    '';

    buildPhase = ''
      runHook preBuild
      export CARGO_PROFILE_RELEASE_LTO=false
      cargo build --lib --target ${cargoTarget} --release --no-default-features --features pulley
      runHook postBuild
    '';

    installPhase = ''
      mkdir -p $out/lib $out/include
      if [ -f target/${cargoTarget}/release/libwawona_wasm.a ]; then
        cp target/${cargoTarget}/release/libwawona_wasm.a $out/lib/
      else
        echo "ERROR: libwawona_wasm.a not found" >&2
        find target -name 'libwawona_wasm*' >&2 || true
        exit 1
      fi
      cp ${wasmSrc}/source/include/wawona_wasm.h $out/include/ 2>/dev/null || true
    '';
  }
