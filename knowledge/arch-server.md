# The Rust server

`server-rs/` builds `skibidink`, one axum process that is the whole backend: it owns the port,
composes every page, serves `public/` and answers every API route. There is no Node at runtime;
new backend work goes into `server-rs/`. One compiled binary keeps memory low and flat, which is
what lets the site run on a free or small instance.

```
Cloudflare (router/) → skibidink :$PORT ─┬─ middleware (web.rs)   edge check, cross-site guard, rate limit
                                          ├─ site routes (site.rs) /, /book, /api/ready, /api/health
                                          ├─ booking (book.rs, admin.rs, pay.rs) /api/availability, /api/reservations,
                                          │                         /admin, /api/admin/*, /api/pay/paymongo
                                          ├─ accounts (account.rs)  /account, /api/account/*
                                          └─ fallback             /_v/<hash>/…, public/ files, else 302 /
```

## Modules

| File | Holds |
| --- | --- |
| `main.rs` | `App`, the `.env` loader, the router, the sweeper, shutdown |
| `web.rs` | `Ctx`, the middleware, the limiter, token buckets, `Fail`, response helpers, ETags |
| `pages.rs` | `Meta`, the shared `<head>`, structured data, `versioned()`, `compose()`, the page cache |
| `public.rs` | Files under `public/`: lookup, content types, the asset tree hash |
| `site.rs` | Page routes, health, the fallback |
| `book.rs` | Reservations: the schema, validation, `confirm`, the public routes (`arch-booking.md`) |
| `admin.rs` | Admin sign-in and every `/api/admin` route (`auth-admin.md`, `arch-booking.md`) |
| `pay.rs` | PayMongo checkout and webhook (`integ-paymongo.md`) |
| `account.rs` | Customer accounts: register, verify, sign in, reset, profile (`auth-account.md`) |
| `mail.rs` | The one way to send an email, provider chosen by env (`integ-mail.md`) |
| `poster.rs` | Schedule pictures as PNG (`design-booking-posters.md`) |
| `date.rs` | Calendar days without a date crate, Manila now, hour labels |
| `db.rs` | The Postgres pool (`deadpool-postgres`, TLS without certificate checks), the app's own schema |

## Boot

1. `.env` under `SKIBIDINK_ROOT` (default `.`) is read for a local run. **A variable already set
   always wins**, and the image ships no `.env`, so production reads only the platform's variables.
2. Postgres connects when `DATABASE_URL` is set and `book::SCHEMA` then `account::SCHEMA` run
   (the second alters `reservations`, so the order matters). The batch runs as one transaction: one
   failing statement rolls back all of it.

**Every connection works inside its own schema**, `DB_SCHEMA` (default `skibidink`): the pool's
`post_create` hook runs `CREATE SCHEMA IF NOT EXISTS` and `SET search_path` on each new connection.
The database is shared with other apps and already holds unrelated tables such as a `users` with
different columns; without the schema `CREATE TABLE IF NOT EXISTS users` silently keeps the
foreign table and every query against it fails. The session pooler (port 5432) keeps the
`search_path` for the connection's life; a transaction pooler (port 6543) would not. Every statement there must be
   `CREATE … IF NOT EXISTS`, because it runs on every boot. A missing database never stops the boot:
   pages serve, and routes that call `App::db()` answer 500.
3. The port binds; `/api/ready` answers from then on.

The release profile unwinds on panic. **`panic = "abort"` turns one bad request into a dead
process.**

SIGTERM or Ctrl-C stops accepting connections and drains for at most 10 seconds.

## The middleware, in order

`web::middleware` wraps the whole router (it is not layered on it), so the path it rewrites is the
path routing sees.

1. **Edge check.** `x-skibidink-client-ip` and `x-skibidink-host` are trusted only when
   `x-skibidink-edge` equals `EDGE_SECRET` (constant-time compare). Otherwise the client is the last
   `x-forwarded-for` entry, then the socket peer. Trusting the headers bare lets anyone pick their
   address and reset their rate limit.
2. **Cross-site write guard.** Every POST/PUT/PATCH/DELETE is 403 unless `Sec-Fetch-Site` is
   `same-origin` or `none`; without that header, an `Origin` must match `Host`; with neither, it is
   allowed (curl carries no victim's cookies). This stands in for CSRF tokens.
3. **Rate limit.** `RATE_LIMIT` (default 100) points per second per address; a static-extension
   path costs 1, anything else 5. At most 20 000 addresses are tracked at any instant.
4. **Trailing slash.** A single trailing slash is dropped for routing.

After the handler: HSTS in production (no `includeSubDomains`, no `preload`), `nosniff`,
`strict-origin-when-cross-origin`, `SAMEORIGIN`, and every `/api` answer without its own
`Cache-Control` gets `no-store` and `noindex`.

`web::Buckets` is a per-address token bucket for routes that need their own limit
(`request_buckets` for booking requests, `login_buckets` for admin sign-in, `account_buckets` for
customer sign-in and registration). It starts full and refills continuously.

## Errors

A handler returns `web::Res`. `Fail("text")` answers 500 with that text. A `String` error
converted with `?` is marked internal: it is logged and the client sees "Something went wrong.",
because database errors name tables and constraints.

## Revalidation

`web::etag` sits **inside** `CompressionLayer` and tags every GET/HEAD 200 with a known size up to
4 MB with a weak ETag (SHA-1 of the body); a matching `If-None-Match` gets 304. Moved outside the
compression layer, every body is a stream and nothing is tagged. Pages are `private, no-cache`, so
a reload of an unchanged page costs a few hundred bytes.

## Page composition

Pages in `public/*.html` are **body fragments** (`<body>…</html>`). `Pages::compose(meta, body)`
prepends the shared head built from a `Meta` (title, description, canonical, robots, Open Graph,
JSON-LD), versions every asset, and strips HTML comments outside `<script>`, `<style>`, `<pre>` and
`<textarea>`. Nothing is minified; `CompressionLayer` (brotli, else gzip) does the shrinking.

Each route builds its own `Meta`, so every page has its own title and canonical. One shared set
would canonicalise every page to the homepage.

The structured data states only true, on-site facts: the owner's phone, email, Facebook
(`sameAs`) and map pin (`hasMap`) are in it. **No street address or opening hours until the owner
confirms them**: fabricated local-business data risks a Google manual action.

`SITE_URL` (default `https://skibidink.com`) is the canonical origin. It must match the domain the
site is served on, `public/robots.txt` and `public/sitemap.xml`.

`Pages::cached(key, build)` composes a page once per process in production and on every request
otherwise, so an edit shows on reload in development. **The key must cover every input that varies
the output**; anything per-request must never go through the cache.

## Cache busting

Every asset in `public/` is served one year immutable in production, so only a URL change reaches a
returning visitor. In production `versioned()` rewrites every root-relative `href`/`src` to a
static file into `/_v/<hash>/<path>`, where `<hash>` is `public::tree()`: 12 hex characters of a
SHA-256 over every non-HTML, non-dot file in `public/`, computed once at boot.

- One hash for the whole tree, so files that reference each other relatively still resolve.
- `/_v/<hash>/…` is answered only for the current hash; any other hash is 404, because today's bytes
  under another hash would be cached for a year.
- Outside production, and for `sw.js`, the plain path plus `?v=<deploy>` is used.
- **`public::tree()` must hash exactly like `router/deploy.sh`** (`ops-edge.md` § Versions).

## Static files

`public::lookup` serves only the extensions in `TYPES`, never `.html`, never a dot segment, and
checks every segment after percent-decoding so `%2e%2e` cannot climb out. `TYPES` must stay the
same list as `STATIC` in `router/src/assets.js`.

## Running it

- `npm run dev` (`bin/dev`): Tailwind in watch mode plus `cargo run`.
- `npm run server` then `npm start`: the release build as production runs it.
- `npm test`: the Rust unit tests.

`bin/css` downloads the pinned Tailwind standalone binary into `.bin/` and checks its SHA-256 before
running it. `cargo` comes from rustup; the npm scripts put `~/.cargo/bin` on `PATH`.
