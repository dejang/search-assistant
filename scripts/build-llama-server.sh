#!/usr/bin/env bash
set -euo pipefail

version="v0.7.0"
sha256="55d67acf3ad2ad747a9bbeb75f9f0e173dbcc0d1753a471038a90c819421c5e1"
root="$(cd "$(dirname "$0")/.." && pwd)"
target="${1:-$(rustc -vV | sed -n 's/^host: //p')}"
out="$root/src-tauri/binaries/llama-server-$target"
stamp="$(shasum -a 256 "$0" | cut -d ' ' -f 1)"

case "$target" in
  aarch64-apple-darwin) arch=arm64 ;;
  x86_64-apple-darwin) arch=x86_64 ;;
  *) echo "llama-server is built for macOS only, not $target" >&2; exit 1 ;;
esac

if [ -x "$out" ] && [ "$(cat "$out.version" 2>/dev/null)" = "$stamp" ]; then
  exit 0
fi

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

curl -fsSL -o "$work/src.tar.gz" "https://github.com/ggml-org/llama.cpp/archive/refs/tags/$version.tar.gz"
echo "$sha256  $work/src.tar.gz" | shasum -a 256 -c -
mkdir "$work/src"
tar -xzf "$work/src.tar.gz" -C "$work/src" --strip-components 1

cmake -S "$work/src" -B "$work/build" \
  -DCMAKE_BUILD_TYPE=Release \
  -DCMAKE_OSX_ARCHITECTURES="$arch" \
  -DCMAKE_OSX_DEPLOYMENT_TARGET=13.3 \
  -DBUILD_SHARED_LIBS=OFF \
  -DGGML_NATIVE=OFF \
  -DGGML_METAL=ON \
  -DGGML_METAL_EMBED_LIBRARY=ON \
  -DLLAMA_OPENSSL=OFF \
  -DLLAMA_USE_PREBUILT_UI=OFF \
  -DLLAMA_BUILD_TESTS=OFF \
  -DLLAMA_BUILD_EXAMPLES=OFF
cmake --build "$work/build" --target llama-server -j "$(sysctl -n hw.ncpu)"

mkdir -p "$(dirname "$out")"
cp "$work/build/bin/llama-server" "$out.tmp"
mv -f "$out.tmp" "$out"
echo "$stamp" > "$out.version"
