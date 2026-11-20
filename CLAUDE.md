# skibidink

Working rules (documentation, comments, `knowledge/` naming, testing) live in the global
`~/.claude/CLAUDE.md` and apply here unchanged. `knowledge/README.md` is the index.

## Shape

- `server-rs/` — the Rust (axum) server, the whole backend. `knowledge/arch-server.md`.
- `bin/static` + `.github/workflows/pages.yml` — the site is published on GitHub Pages from `main`.
  `knowledge/ops-pages.md`.
- `router/` — a Cloudflare Worker, **not deployed**. Never deploy to Cloudflare unless the user asks.
- `public/` — pages are `<body>` fragments the server wraps with the shared head.
- `config/input.css` — Tailwind v4, compiled by `bin/css`.

## Before reporting a deploy done

A push is not a live site. Wait for the Pages workflow run for that commit to succeed, then fetch
the page from the Pages URL and check the commit appears in its `?v=` asset URLs. Say "pushed, not
live yet" until it does.

## Placeholders

Location, contact and booking are placeholders. Never invent an address, phone number, price or
opening hours; the owner supplies them.
