#!/bin/sh
# Baut den FFI-Crate einer Qu-App für Android (.so nach app/src/main/jniLibs) und erzeugt die Kotlin-Anbindung.
# Aufruf aus dem android/-Ordner der App (Gradle preBuild): sh <qu-core>/android/build-rust.sh <crate>, z. B. aniqu-ffi.
# Braucht: rustup mit Android-Zielen, cargo-ndk, NDK. Standard: alles unter ~/Android (ohne root, eigenes rustup,
# damit das System-Rust unberührt bleibt). Ziele: ABIS="arm64-v8a x86_64" (x86_64 für Waydroid/Emulator).
set -e
crate=$1
lib=$(echo "$crate" | tr - _)
SDK=${ANDROID_HOME:-$HOME/Android/Sdk}
export ANDROID_NDK_HOME=${ANDROID_NDK_HOME:-$(ls -d "$SDK"/ndk/* | sort -V | tail -n1)}
export RUSTUP_HOME=${RUSTUP_HOME:-$HOME/Android/rust/rustup}
export PATH="$HOME/Android/rust/cargo/bin:$PATH"
export CARGO_TARGET_DIR=../target/android
PROFILE=${PROFILE:-release}

args=""
for abi in ${ABIS:-arm64-v8a x86_64}; do args="$args -t $abi"; done
# bindgen (z. B. rquickjs in TubeQu) gibt clang das Ziel ohne API-Level; die NDK-Header lehnen das ab
# („Unversioned target triples“, es fehlt __ANDROID_MIN_SDK_VERSION__; 26 = minSdk der Apps). cargo-ndk setzt nur
# BINDGEN_EXTRA_CLANG_ARGS_<ziel_mit_unterstrichen>; bindgen liest die Form mit Bindestrichen zuerst – daher hier
# dieselben Pfade wie cargo-ndk plus die Definition (Variablen mit „-“ im Namen nur über `env`).
sysroot="$ANDROID_NDK_HOME/toolchains/llvm/prebuilt/linux-x86_64/sysroot"
bg() { echo "BINDGEN_EXTRA_CLANG_ARGS_$1=--sysroot=$sysroot -I$sysroot/usr/include/$1 -D__ANDROID_MIN_SDK_VERSION__=26"; }
# shellcheck disable=SC2086
env "$(bg aarch64-linux-android)" "$(bg x86_64-linux-android)" \
  cargo ndk $args -o app/src/main/jniLibs build -p "$crate" --profile "$PROFILE"

# Kotlin-Code aus der Bibliothek (Host-Build reicht, die Schnittstelle ist überall gleich).
cargo build -q -p "$crate"
cargo run -q -p "$crate" --bin uniffi-bindgen -- generate --library "$CARGO_TARGET_DIR/debug/lib$lib.so" \
  --language kotlin --out-dir app/src/main/java --no-format
