#!/bin/sh
# See knowledge/ops-edge.md. Usage: router/deploy.sh [ref] [local|<base url>]
set -eu

repo=$(cd "$(dirname "$0")/.." && pwd)
gitdir=$(git -C "$repo" rev-parse --absolute-git-dir)
tree="$gitdir/router-tree"
lock="$gitdir/router-deploy.lock"
work=$(mktemp -d)
pid=
locked=

cleanup() {
  [ -n "$pid" ] && kill "$pid" 2>/dev/null || true
  git -C "$repo" worktree remove --force "$tree" >/dev/null 2>&1 || true
  rm -rf "$work"
  [ -n "$locked" ] && rmdir "$lock" 2>/dev/null || true
}
trap cleanup EXIT INT TERM

i=0
until mkdir "$lock" 2>/dev/null; do
  i=$((i + 1)); [ $i -gt 360 ] && { echo "a router deploy has held $lock for 30 minutes" >&2; exit 1; }
  sleep 5
done
locked=1

ref=${1:-HEAD}
source=${2:-local}
commit=$(git -C "$repo" rev-parse "$ref")

if [ "$source" != local ] && [ -z "${EDGE_SECRET:-}" ]; then
  echo "EDGE_SECRET must be set: without it the live site answers with the snapshots being replaced." >&2
  exit 1
fi

git -C "$repo" worktree remove --force "$tree" >/dev/null 2>&1 || rm -rf "$tree"
git -C "$repo" worktree prune
git -C "$repo" worktree add --detach "$tree" "$commit" >/dev/null 2>&1
mkdir -p "$repo/.bin" && ln -s "$repo/.bin" "$tree/.bin"
(cd "$tree" && bin/css --minify >/dev/null 2>&1)

dist="$tree/router/dist"
rm -rf "$dist"
mkdir -p "$dist/_fallback"
rsync -a --exclude='*.html' --exclude='.*' "$tree/public/" "$dist/"
# DANGER: must produce the same hash as tree() in server-rs/src/public.rs. A different hash sends
# every asset of every page to the origin instead of Cloudflare.
assets=$(cd "$tree/public" && find . -type f ! -name '*.html' ! -path '*/.*' | LC_ALL=C sort |
  while IFS= read -r f; do shasum -a 256 "$f"; done | shasum -a 256 | cut -c1-12)
mkdir -p "$dist/_v/$assets"
rsync -a --exclude='*.html' --exclude='.*' "$tree/public/" "$dist/_v/$assets/"
# DANGER: both rules name this hash only. With all of /_v/*, a file from another hash is answered by
# the asset layer as a 404 marked immutable, and a browser on an older page keeps it for a year.
cat >"$dist/_headers" <<HEADERS
/_v/$assets/*
  Cache-Control: public, max-age=31536000, immutable
  X-Content-Type-Options: nosniff
  Strict-Transport-Security: max-age=31536000
HEADERS
sed -i '' "s|^run_worker_first = true\$|run_worker_first = [\"/*\", \"!/_v/$assets/*\"]|" "$tree/router/wrangler.toml"
grep -q "!/_v/$assets/" "$tree/router/wrangler.toml" || { echo "run_worker_first was not rewritten" >&2; exit 1; }

if [ "$source" = local ]; then
  (cd "$tree/server-rs" && CARGO_TARGET_DIR="$repo/server-rs/target" PATH="$HOME/.cargo/bin:$PATH" cargo build --release -q)
  port=3918
  # DANGER: env -i keeps this run off the real database. A DATABASE_URL inherited from the shell
  # would run the boot schema against production from a laptop.
  env -i PATH="$PATH" HOME="$HOME" NODE_ENV=production RENDER_GIT_COMMIT="$commit" PORT=$port \
    SKIBIDINK_ROOT="$tree" SITE_URL="${SITE_URL:-}" DATABASE_URL= "$repo/server-rs/target/release/skibidink" >"$work/server.log" 2>&1 &
  pid=$!
  i=0
  until curl -sf "http://127.0.0.1:$port/api/ready" >/dev/null 2>&1; do
    i=$((i + 1)); [ $i -gt 60 ] && { cat "$work/server.log"; exit 1; }
    sleep 1
  done
  source="http://127.0.0.1:$port"
fi

named_by() {
  [ "$(grep -o '/_v/[0-9a-f]*/' "$1" | sort -u)" = "/_v/$assets/" ]
}

for path in $(grep -o '<loc>[^<]*' "$tree/public/sitemap.xml" | sed 's|<loc>https\{0,1\}://[^/]*||'); do
  name=$(printf '%s' "$path" | sed 's|/*$||')
  out="$dist/_fallback${name:-/index}.html"
  : >"$work/h"
  curl -s -D "$work/h" -o "$work/b" -H "x-skibidink-origin: ${EDGE_SECRET:-}" "$source$path" || true
  head -1 "$work/h" | grep -q ' 200' || { echo "skip $path: $(head -1 "$work/h")"; continue; }
  # SECURITY: a snapshot is handed to every visitor. A page that set a cookie carries something tied
  # to one session, and a page marked noindex is private.
  grep -qiE '^(set-cookie|x-skibidink-snapshot):' "$work/h" && { echo "skip $path: session header"; continue; }
  grep -qiE '^x-robots-tag:.*noindex' "$work/h" && { echo "skip $path: noindex"; continue; }
  named_by "$work/b" || { echo "skip $path: names assets other than /_v/$assets/"; continue; }
  mkdir -p "$(dirname "$out")"
  mv "$work/b" "$out"
  echo "page $path"
done
echo "pages: $(find "$dist/_fallback" -name '*.html' | wc -l | tr -d ' ')"

[ -n "$pid" ] && kill "$pid" 2>/dev/null || true
pid=
cd "$tree/router"
[ -d "$repo/router/node_modules" ] || (cd "$repo/router" && npm install --silent --no-audit --no-fund)
"$repo/router/node_modules/.bin/wrangler" deploy --var "ASSETS_VERSION:$commit"
