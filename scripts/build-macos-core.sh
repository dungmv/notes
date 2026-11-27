#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"

package="$root/clients/macos/Packages/NotesCore"
target="$root/target"
export MACOSX_DEPLOYMENT_TARGET=14.0

cargo build --locked --release --package notes-ffi --lib --target-dir "$target"
library="$target/release/libnotes_ffi.a"

stage="$(mktemp -d "$target/macos-core.XXXXXX")"
trap 'rm -rf "$stage"' EXIT
mkdir -p "$stage/Headers"

cargo run --locked --package notes-ffi --features bindgen --bin uniffi-bindgen-swift \
    --target-dir "$target" -- "$library" "$stage/Headers" --swift-sources --headers \
    --modulemap --module-name NotesCoreFFI \
    --modulemap-filename module.modulemap --metadata-no-deps
mv "$stage/Headers/notes_ffi.swift" "$stage/"

xcodebuild -create-xcframework -library "$library" -headers "$stage/Headers" \
    -output "$stage/NotesCoreFFI.xcframework"

mkdir -p "$package/Sources/NotesCore"
cp "$stage/notes_ffi.swift" "$package/Sources/NotesCore/"
rm -rf "$package/NotesCoreFFI.xcframework"
mv "$stage/NotesCoreFFI.xcframework" "$package/"
