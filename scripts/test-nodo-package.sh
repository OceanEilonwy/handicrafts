#!/bin/sh
# Checks the monokulo-nodo feed app (packaging/nodo) with nodo's own parsers: the manifest through
# the feed's validation and install review, the settings through the device's Lua loader, and the
# launcher with a stand-in for the shop.
#
# Usage: scripts/test-nodo-package.sh NODO_CHECKOUT
#   NODO_CHECKOUT is a monokulo-nodo checkout, such as the one at NODO_REV in
#   .github/workflows/feed.yml. The test is copied into it and removed afterwards.
set -eu

root=$(cd "$(dirname "$0")/.." && pwd)
nodo=${1:?usage: scripts/test-nodo-package.sh NODO_CHECKOUT}
nodo=$(cd "$nodo" && pwd)
[ -d "$nodo/crates/nodo-install/tests" ] || {
	echo "test-nodo-package: $nodo is not a monokulo-nodo checkout" >&2
	exit 1
}

test=$nodo/crates/nodo-install/tests/handicrafts_package.rs
cp "$root/packaging/nodo/tests/handicrafts_package.rs" "$test"
trap 'rm -f "$test"' EXIT
cd "$nodo"
HANDICRAFTS_PACKAGING="$root/packaging/nodo" cargo test --locked -p nodo-install --test handicrafts_package
