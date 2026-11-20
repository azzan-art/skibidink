# skibidink

Working rules (documentation, comments, `knowledge/` naming, testing) live in the global
`~/.claude/CLAUDE.md` and apply here unchanged. `knowledge/README.md` is the index.

## Shape

- `server-rs/` — the Rust (axum) server, the whole backend. `knowledge/arch-server.md`.
- `router/` — the Cloudflare Worker in front of it: static assets and page snapshots at the edge.
  `knowledge/ops-edge.md`.
- `public/` — pages are `<body>` fragments the server wraps with the shared head.
- `config/input.css` — Tailwind v4, compiled by `bin/css`.

## Before reporting a deploy done

A push is not a live site. After `router/deploy.sh`, fetch the page from the live URL and check
`x-skibidink-snapshot` equals the commit. Say "pushed, not live yet" until it does.

## Placeholders

Location, contact and booking are placeholders. Never invent an address, phone number, price or
opening hours; the owner supplies them.
