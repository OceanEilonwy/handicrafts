#!/bin/sh
# Makes what the monokulo-nodo feed is built from, for one commit (default HEAD), in target/nodo/:
#
#   handicrafts-<commit>.tar.gz  git archive of the commit, plus `cargo vendor` output in
#                                cargo-vendor/ and a .cargo/config.toml pointing cargo at it, so
#                                the build needs no network. Reproducible: the same commit gives
#                                the same bytes.
#   srcpkgs/handicrafts/         packaging/nodo/srcpkgs/handicrafts with @VERSION@, @COMMIT@ and
#                                @SOURCE_SHA256@ filled in.
#
# The version is the crate's version and the commit count, such as 0.2.2.32, so every commit
# upgrades the last, as the OpenWrt package's -r<count> does.
#
# Prints version=… and commit=… lines (the feed workflow appends them to $GITHUB_OUTPUT).
#
# Usage: scripts/nodo-dist.sh [COMMIT]
set -eu

root=$(cd "$(dirname "$0")/.." && pwd)
rev=${1:-HEAD}
out=$root/target/nodo

commit=$(git -C "$root" rev-parse --verify "$rev^{commit}")
crate=$(git -C "$root" show "$commit:Cargo.toml" | sed -n 's/^version = "\([^"]*\)"$/\1/p' | head -n 1)
[ -n "$crate" ] || {
	echo "nodo-dist: no version in Cargo.toml at $commit" >&2
	exit 1
}
count=$(git -C "$root" rev-list --count "$commit")
version=$crate.$count
when=$(git -C "$root" show -s --format=%ct "$commit")
name=handicrafts-$commit

rm -rf "$out"
mkdir -p "$out/tree" "$out/srcpkgs"
git -C "$root" archive --format=tar --prefix="$name/" "$commit" | tar -x -C "$out/tree"

# Vendor from inside the tree, so the printed configuration names cargo-vendor relatively.
(
	cd "$out/tree/$name"
	mkdir -p .cargo
	cargo vendor --locked --versioned-dirs cargo-vendor >"$out/vendor-config.toml"
)
grep -q '^directory = "cargo-vendor"$' "$out/vendor-config.toml" || {
	echo "nodo-dist: cargo vendor printed no relative cargo-vendor directory" >&2
	exit 1
}
if [ -e "$out/tree/$name/.cargo/config.toml" ]; then
	echo "nodo-dist: the repository has its own .cargo/config.toml; merge the vendor config into it" >&2
	exit 1
fi
mv "$out/vendor-config.toml" "$out/tree/$name/.cargo/config.toml"

# The same bytes for the same commit: sorted names, one owner, the commit's time, no gzip name or
# time.
tar -C "$out/tree" --sort=name --owner=0 --group=0 --numeric-owner --mtime="@$when" \
	--pax-option=exthdr.name=%d/PaxHeaders/%f,delete=atime,delete=ctime -cf - "$name" |
	gzip -n -9 >"$out/$name.tar.gz"
rm -rf "$out/tree"
sum=$(sha256sum "$out/$name.tar.gz" | cut -d' ' -f1)

git -C "$root" archive --format=tar "$commit" packaging/nodo/srcpkgs | tar -x -C "$out"
mv "$out/packaging/nodo/srcpkgs/"* "$out/srcpkgs/"
rm -rf "$out/packaging"
template=$out/srcpkgs/handicrafts/template
sed -i -e "s/@VERSION@/$version/g" -e "s/@COMMIT@/$commit/g" -e "s/@SOURCE_SHA256@/$sum/g" "$template"
if grep -q '@[A-Z_]*@' "$template"; then
	echo "nodo-dist: placeholders left in $template:" >&2
	grep -n '@[A-Z_]*@' "$template" >&2
	exit 1
fi

echo "version=$version"
echo "commit=$commit"
