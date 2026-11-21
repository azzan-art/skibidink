# The in-browser demo

`public/js/demo.js`. On GitHub Pages there is no server, so the site runs as a click-through demo
of the whole flow: board, account home, requests, admin, taken notices, blocking. Nothing in it
reaches a server or a database.

## When it loads

`Pages::new(root, demo)` gets `demo = db.is_none() && !production()`, and the shared head then
loads `<script src="/js/demo.js">` before `site.js`. The Pages build (`bin/static`, `ops-pages.md`)
runs the server that way, and so does a local run without `DATABASE_URL`. A production server
whose database is missing never loads it: real visitors must never "book" on fake data.

The script is not deferred, so it replaces `window.fetch` before any page script runs. A request to
the same origin whose path (minus the base path) starts with `/api/` is answered in the browser;
anything else goes to the real `fetch`.

## Base path

Pages serves the site under `/skibidink`. `demo.js` takes the base from its own `src` (rewritten
by `bin/static`), sets `window.SITE_BASE`, and strips it from `/api/` paths. Page scripts build
navigation with `sitePath('/…')` (`site.js`).

## The fake API

It mirrors the routes in `arch-booking.md` and `auth-account.md` closely enough for the pages to
run unchanged: availability, account me/login/register/logout/profile/reservations, the request
(blocks joined as in `book::picked_blocks`, the 409 on confirmed hours, the 403 when blocked), and
the admin list (with `competing`), walk-ins, status changes, calendar, stats and blocking.
Confirming marks overlapping open requests taken and writes an email to a demo outbox, as the
server does. Differences:

- Any password signs in. Registering signs in at once (`{demo: true}`; `account.js` then shows the
  home instead of "check your email").
- Online payment is off and the calendar picture cannot download (the link opens a note instead).
- A route it does not know answers 404 "The demo does not have this one". A new API route needs its
  demo twin, or that page breaks on Pages.

## Data

Everything lives in `localStorage` under `skibidink.demo.v1`, per browser, seeded on first load
from a fixed seed relative to today: 150 days of past confirmed bookings (for the dashboard), this
week's bookings with one fully booked day, a Demo Player with a confirmed booking, an open request
that competes with another player's awaiting one, and an already taken request, plus one blocked
player. The session (player and admin) is kept there too, so it survives page changes.

## On screen

- A pill at the bottom left: Demo · Book · Account · Admin · Emails (the outbox, with its count) ·
  Reset (clears the key and reloads with fresh seed data).
- "Continue as Demo Player" above the account sign-in, and "Open the demo admin" above the admin
  password: both skip sign-in and reload.
