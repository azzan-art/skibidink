# The Cloudflare router Worker

**Not deployed.** The site is published on GitHub Pages (`ops-pages.md`); nothing pushes to
Cloudflare and no git hook is installed. This file describes `router/` for when the site moves
behind Cloudflare again.

`router/` is a Cloudflare Worker with static assets (Cloudflare's successor to Pages). It answers
every static file in `public/` and a rendered snapshot of every page in the sitemap itself, so
visitors to those never reach the Rust origin. Everything else (APIs, future booking and account
pages, a page asked for with a query) is proxied to `ORIGIN`.

```
request ─┬─ /_v/<this hash>/…           Cloudflare asset layer, no Worker run, 1 year immutable
         ├─ static file                  Worker → ASSETS
         ├─ sitemap page, plain query    Worker → ASSETS /_fallback/<path>.html
         └─ anything else                Worker → ORIGIN (Render), snapshot again on 5xx
```

## Settings

| Where | Name | Purpose |
| --- | --- | --- |
| `wrangler.toml` `[vars]` | `ORIGIN` | Base URL of the Rust server. Empty: snapshots and assets only, any other GET 302s to `/` |
| secret | `EDGE_SECRET` | Same value as the origin's `EDGE_SECRET`; proves a proxied request came through the Worker |
| deploy var | `ASSETS_VERSION` | The commit the assets came from; set by `deploy.sh` |

The Worker sends the origin `x-skibidink-edge`, `x-skibidink-client-ip` (from `cf-connecting-ip`)
and `x-skibidink-host`. Without the secret the origin ignores the other two and rate-limits by the
Worker's address.

## Static files

`isStatic` in `router/src/assets.js` mirrors `public::lookup`: the extension in `STATIC` (must equal
`TYPES` in `server-rs/src/public.rs`), no `.html`, no empty or dot segment, never `/_fallback/*`.

## Versions

`deploy.sh` hashes the commit's `public/` the same way `public::tree()` does and copies `public/`
twice: at its own paths and under `dist/_v/<hash>/`. It writes `dist/_headers` for that hash and
rewrites `run_worker_first` to `["/*", "!/_v/<hash>/*"]` in its checkout, so those paths are served
without running the Worker.

- **The shell hash and `public::tree()` must agree**, or every asset of every page goes to the origin.
- **Both rules name this hash only, never `/_v/*`.** A path excluded from the Worker that the assets
  do not hold is a 404 carrying the immutable header, and a browser on an older page keeps it a year.
- A `/_v/<other hash>/…` or `?v=<other>` request gets this deploy's file sent `no-cache`, never
  immutable: the bytes are not the ones that URL names.

## Snapshots

`deploy.sh` reads every `<loc>` in `public/sitemap.xml` and saves the page as
`dist/_fallback/<path>.html` (`/` → `index.html`). A page is kept only when it answers 200, sets no
cookie, is not `noindex`, and names `/_v/<hash>/` with this deploy's hash and no other.

**A snapshot is handed to every visitor**, so only a page that is identical for every visitor may
be in the sitemap. A page that reads a session must stay out of it.

A request uses its snapshot when it is GET/HEAD and its query holds only `utm_*`, `fbclid` or
`gclid`. Answers are `public, max-age=0, must-revalidate` with `x-skibidink-snapshot: <commit>`.
`x-skibidink-origin: <EDGE_SECRET>` skips the snapshot (how a remote page source is read).

A snapshot is only as new as the last `deploy.sh`; a change to a page reaches visitors when the
Worker is deployed, not when Render is.

## Deploying

`router/deploy.sh [ref] [local|<base url>]` (defaults `HEAD`, `local`):

1. checks the commit out into `.git/router-tree`, never the working copy, so uncommitted edits
   never ship;
2. builds `output.css` there and fills `router/dist/` as above;
3. `local` builds that commit's server and runs it with `env -i` and no `DATABASE_URL`, so a
   snapshot run can never touch the real database; a base URL reads that site instead and needs
   `EDGE_SECRET` in the environment;
4. runs `wrangler deploy --var ASSETS_VERSION:<commit>` with the account from `wrangler login`.

A lock (`.git/router-deploy.lock`) runs one deploy at a time. `router/pre-push`, when linked as
`.git/hooks/pre-push`, starts the script in the background for every push to `main`, logs to
`.git/router-deploy.log` and posts a macOS notification. It is not linked while Pages is the host.

**A bare `wrangler deploy` drops `ASSETS_VERSION` and the `/_v/` tree**; every versioned asset then
falls through to the origin.

The script is read by `sh` as it runs; editing `deploy.sh` during a deploy breaks that deploy.
