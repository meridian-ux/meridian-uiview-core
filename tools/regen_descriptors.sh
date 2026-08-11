#!/usr/bin/env bash
# Regenerate BOTH committed descriptor sets — the cargo build's inputs:
#
#   rust/uiview/proto/gen/meridian_ui_v1.binpb        <- //proto:uiview_proto
#   rust/theme-proto/proto/gen/meridian_theme_v1.binpb <- //proto:theme_proto
#
# Two sets, not one, because Bazel already makes them two crates: `uiview_proto`
# and `theme_proto` are separate rust_prost_library targets, and meridian-tui
# names `theme_proto` by that exact path. Folding theme into the ui set would
# make the cargo and Bazel module paths differ, which is the one thing the
# dual-build idiom exists to prevent. It is also the honest boundary — semantics
# and style are orthogonal, which is why they were split upstream.
#
# WHY A COMMITTED SET. Bazel gets its types from
# `@meridian_schemas//proto:uiview_proto` through rust_prost_library. cargo has no
# such edge: meridian-schemas is a different repository, so without a checked-in
# descriptor set `cargo build` cannot work here at all — and, transitively,
# meridian-tui cannot be built or run either. This is the same trade
# meridian-proto makes (see its build.rs), for the same reason.
#
# THE SET MUST MATCH THE bazel_dep. If MODULE.bazel says meridian_schemas 0.24.0,
# this must be generated from 0.24.0. If they disagree, cargo and Bazel generate
# DIFFERENT Rust types from the same crate — which is precisely the class of bug
# the estate has been fighting (see meridian-k8s, whose vendored copy drifted from
# 0.5.0 while canonical moved on).
#
# Usage:
#   tools/regen_descriptors.sh [path-to-meridian-schemas]
#
# Needs protoc (any 33.x; the estate pins 33.4 in bazel/protoc_prebuilt.bzl) and
# googleapis' field_behavior.proto. google/protobuf/struct.proto ships with protoc.
set -euo pipefail

SCHEMAS="${1:-../meridian-schemas}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT="$HERE/rust/uiview/proto/gen/meridian_ui_v1.binpb"
THEME_OUT="$HERE/rust/theme-proto/proto/gen/meridian_theme_v1.binpb"

[ -d "$SCHEMAS/proto" ] || { echo "no proto/ under $SCHEMAS — pass the meridian-schemas checkout as \$1" >&2; exit 1; }

# The file list is derived from proto/BUILD.bazel's `uiview_proto` proto_library,
# NOT hand-maintained here — a second list would drift against the first, which is
# the exact failure mode meridian-uiview-core's BUILD.bazel comment warns about.
SRCS="$(python3 - "$SCHEMAS" <<'PY'
import re, sys, pathlib
build = pathlib.Path(sys.argv[1], "proto", "BUILD.bazel").read_text()
blk = re.search(r'proto_library\(\s*name = "uiview_proto".*?\)\n', build, re.S).group(0)
print(" ".join("proto/" + p for p in re.findall(r'"([^"]+\.proto)"', blk)))
PY
)"

# googleapis is a Bazel module under Bazel; for the cargo path we need the one
# file the schemas actually import.
GAPI="$(mktemp -d)"
mkdir -p "$GAPI/google/api"
curl -fsSL -o "$GAPI/google/api/field_behavior.proto" \
  "https://raw.githubusercontent.com/googleapis/googleapis/master/google/api/field_behavior.proto"

mkdir -p "$(dirname "$OUT")"
( cd "$SCHEMAS" && protoc --include_imports --descriptor_set_out="$OUT" -I . -I "$GAPI" $SRCS )

# theme.proto is its own proto_library and its own crate. It imports nothing
# today, so no -I "$GAPI" is needed — but --include_imports stays, because the
# guard in rust/theme-proto/tests/contract.rs asserts self-containment and an
# import added upstream should widen the set rather than break codegen.
( cd "$SCHEMAS" && protoc --include_imports --descriptor_set_out="$THEME_OUT" -I . -I "$GAPI" proto/theme.proto )

rm -rf "$GAPI"

echo "wrote $OUT ($(stat -c%s "$OUT") bytes) from $(echo "$SRCS" | wc -w) protos"
echo "wrote $THEME_OUT ($(stat -c%s "$THEME_OUT") bytes) from proto/theme.proto"
echo "Now run: (cd rust && cargo test --workspace --all-targets)"
