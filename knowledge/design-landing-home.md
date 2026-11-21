# The homepage

`public/index.html`, composed by `site::home`. The layout follows a clean sports-venue landing
page: a frosted fixed nav, a hero over a faded court, and alternating light sections with cards.
Colours come from the SkibiDink logo.

## Copy

The copy never states how many courts there are: no "one court", "a single court" or "courts". It
says "pickleball", "a place to play" or "the court" in passing. The tone is plain and
factual: no superlatives or marketing claims ("tournament-grade", "next level", "friendly community").
Facts come only from the owner. Supplied so far: daily hours 6 AM to 11 PM, paddle and ball rental
(first come, first served, ask staff), non-refundable bookings, parking at or inside the main gate,
and the four house rules and six FAQ answers as worded by the owner. Anything else (address, contact,
surface, lighting) is "coming soon", never described.

## Type

| Role | Font | Files |
| --- | --- | --- |
| Headings (`font-display`) | Outfit 300–900 | `public/vendor/fonts/outfit-latin*.woff2` |
| Body (`font-sans`) | Plus Jakarta Sans 300–700 | `public/vendor/fonts/jakarta-latin.woff2` |

All fonts are self-hosted and preloaded; no request goes to Google Fonts.

## Palette (`@theme` in `config/input.css`)

| Token | Value | Use |
| --- | --- | --- |
| `dink` | `#0b3fae` | Primary buttons, heading accents (the logo's "Dink" blue) |
| `skibi` | `#00a04b` | The logo's "Skibi" green, sampled from `logo.png`: eyebrows, pills, and every green on the booking board (selected day and hour, Request button, open markers) |
| `skibi-dark` | `#007a39` | Court surround on the floor plan, Hold button hover |
| `ball` | `#a4dc4c` | The logo's ball green: only the ball itself. Greens elsewhere are `skibi`, never a generic Tailwind lime or emerald, so the page matches the logo |
| `ink` / `body` / `muted` | slate 900 / 800 / 500 | Headings / text / secondary text |
| `paper` / `line` | `#f8fafc` / `#e2e8f0` | Page background / borders |
| `mist` | `#eef2f7` | The `/book` page's band: a very light blue-gray, never a dark band (the owner finds near-black too heavy) |

## Sections

| id | Content |
| --- | --- |
| hero | The playing logo as the `<h1>` (with an `sr-only` text name, § Playing logo), tagline, two buttons, over the court floor (§ Court floor) |
| `the-place` | The place and its location in one: a gallery on the left (two photo frames over a wide map panel) and the copy plus four cards (hours, booking, address, contact) on the right; on phones the copy comes first and the gallery below it |
| `rules` | Four house-rule cards |
| `faqs` | `<details>` accordion, no script |

Every Book button (hero, topbar) and the FAQ answer about booking link to **`/book`**, its own page
(`public/book.html`, in the sitemap). Booking is for signed-in accounts only (`auth-account.md` §
Signed-out visitors): a signed-out visitor gets "Sign in" / "Sign in to book" links to
`/account?next=/book` in their place, and `/book` itself redirects there. It carries the same topbar without the Book button (its links
point to `/#the-place`, `/#rules`, `/#faqs`), the booking board (§ Court board) on the light
blue-gray `mist` band with white `line`-bordered panels, and the footer. Its `<h1>` is "Ready to
play?": the last word cycles play? → book? → dink?, 3 s each, sliding up through a clip
(`.word-swap`, one `word-cycle` keyframe of 9 s with the 2nd and 3rd words delayed −6 s and −3 s,
CSS only, still on reduced motion; screen readers get "play?").

**Contact and map are the owner's; photos and the street address are still placeholders.** The
owner's details: phone +63 998 227 4603, email skibidinkofficial@gmail.com, Facebook page
`profile.php?id=61596374805137`, map pin `https://maps.app.goo.gl/mUGWVuJdh1voBKHB6`. They appear in
the homepage cards ("Location", "Inquiries & contact"), in every page footer, and in the structured
data (`arch-server.md`); a change goes to all three. `.map-slot` embeds the pin by its coordinates
(16.8564706, 120.3823384, `output=embed`), not by the place name Google attaches to it. No street
address is shown until the owner gives one. The two `.photo-slot` frames take an `<img>`
(object-cover) when the owner sends photos.

## Court board

`public/js/book.js` draws the venue's one court on a floor plan (court centred above the
entrance, benches and water station) with hourly slots across the opening hours, 6 AM to 11 PM
(`OPEN`, `CLOSE`, the same numbers as `book.rs`). It runs in one of two modes, picked at load:

- **Live**, when `GET /api/availability` answers: an hour is booked when the server says it is
  (confirmed bookings only, `arch-booking.md`), refreshed every minute. Each open hour is a toggle:
  the visitor picks up to 8 (`MAX_PICKS`) across any days, and the summary lists them by day,
  back-to-back hours joined ("4 PM–6 PM, 10 PM–11 PM"). Picks survive switching days and are
  dropped once booked or past. Signed out, the button reads "Sign in to book" and goes to
  `/account?next=/book` (`auth-account.md`). Signed in, "Request N hours" opens the request dialog
  (`#book-dialog`): the picked times, "Booking as <name> · <phone>" with a link to edit them,
  players, note and the payment choice. The times are chosen on the board only; the dialog has no
  duration field. A blocked account gets a disabled "Your account is blocked from booking" button.
  "Pay online now" shows only when
  the server reports online payment on (`integ-paymongo.md`); choosing it sends the visitor to the
  PayMongo checkout, which returns to `/book?paid=<id>` or `/book?unpaid=<id>` and the page shows a
  toast for it. Hours this browser has requested are kept in `localStorage`
  (`skibidink.requested`) and drawn amber.
- **Preview**, when `/api/availability` does not answer at all (the demo, `arch-demo.md`, answers
  it, so this is only a broken server or a page opened without one): a simulated
  schedule. **Every visitor sees the same preview**: whether a slot is booked comes from a hash of
  `date|court|hour` plus a seeded "booked at" moment up to 48 hours before it, never
  `Math.random`, so the board fills as the day goes on. Demand rises at 5–9 PM, 6–8 AM and
  weekends and dips at midday (`demand()`). "Hold N hours" stores the picks in `localStorage`
  only, and the note under the button says it is a preview.

In both modes:

- The page reads Manila time (`Asia/Manila`), not the visitor's clock zone; the clock shows it as
  `1:50 PM`.
- Today's hours that have started are not drawn at all; a pick that slips into the past is cleared.
  Once the last hour has started, today shows "No more hours today".
- A day with no open hour left is a disabled tab with a small rotated "Full" stamp (`.day-stamp`)
  over its date, and the board moves to the next day that has one.
- When the day strip is wider than its panel, a right chevron nudges left and right at its edge
  (`.days-more`, shown by `.can-scroll`); it goes once the strip is scrolled to its end.
- The floor plan panel is hidden below `sm`: on a phone the board is the day strip and the slots.
  Below `sm` the heading is centred and the section starts closer to the topbar.
- "My account" above the heading goes to `/account`, the customer's home (`auth-account.md`).
- The board grid is `grid-cols-1` below `lg`: a grid track defaults to its content's min width, and the
  sideways-scrolling day strip would otherwise push both panels past the edge of a phone screen.
- The board never changes size when the day or the pick changes: `#slots` always holds the full day
  (`min-h-[12.625rem]`: five rows, the full 17 hours; change it with `OPEN`/`CLOSE`) and `#summary`
  always has the height of its three-line "Your pick" state.
- A court booked for the current hour is "in play": players and a ball animate on it. The stats
  show the court's state right now, and the next open hour today.
- There is no "recently booked" feed: the owner does not want activity pills on the page.
- `POS` lists the courts. One court is 72 % × 47.6 % of the floor, which keeps the 44 × 20 ft
  ratio at the floor's 16:11 aspect. With one entry the board says "The court" and draws no name
  label; adding entries brings back per-court labels and the selection outline. The kitchen spans
  34.1 %–65.9 % of the court's length (7 ft each side of the net). The server has no court
  dimension yet: more courts need a `court` column first.

## Playing logo

The logo in the hero, the topbar and the footer is the same playing logo: five stacked layers in
`.logo-play`, all on one 1218 × 230 canvas so they line up with no offsets: `logo-base.png` (the
wordmark with no paddles, ball or trail; the Ski "i", the "D" bowl and the "b" bowl redrawn where they
were covered), `logo-trail.png` (the ball's streak), `logo-paddle-left.png`, `logo-paddle-right.png`
and `logo-ball.png` (60 × 60, positioned by `.logo-ball-x`/`.logo-ball`).

Outside the hero a `.logo-play` gets only a height (`h-7`, `sm:h-8` in the topbar); `aspect-ratio` sets the
width. Never give it a width or a `max-width` as well: with both sides fixed the layers stretch out of
shape on narrow screens.

The paddles on the Ski "i" and the Dink "i" rally over the net (the Skibi "i") on one 12 s loop that
every layer shares. Each shot has its own swing:

| Loop | Ball | Paddles |
| --- | --- | --- |
| 80 % → 24 % (wraps) | **At rest on the net**, trail showing | Still |
| 24 → 30 % | Trail fades; the ball hops to the right paddle | Right: small push (dink) |
| 30 → 41 % | Soft dink, just over the net | Left: big wind-up and follow-through (drive) |
| 41 → 47 % | Fast, flat drive | Right: dips and lifts (lob) |
| 47 → 63 % | High lob, well above the logo | Left: short push |
| 63 → 80 % | Rises along the trail (revealed behind it), overshoots and settles home | Still |

The rest phase keeps the logo legible: for about 5 s of every loop it is exactly the original mark.
**The ball's rest position (`left: 46.06%`, `top: 5.43%`) is its place in the original logo and
doubles as the reduced-motion position**; the spin keyframes end on 360° so the ball's holes land
where they were. Every hit (30, 41, 47, 63 %) has the ball at the paddle face as it sits at that
moment's swing angle, so the keyframe percentages, the swing angles and the 12 s duration are tuned
together; changing one alone puts the hits out of step with the ball.

`transform-origin` on each paddle is its hand in canvas percent (left 28.79 % 35.22 %, right
67.03 % 34.78 %). Paddle keyframes always use `translateY() rotate()` in that order, or the browser
cannot interpolate between them. Re-cutting the layers from a new logo means re-measuring the hands,
the paddle faces and the ball's rest position.

Under `prefers-reduced-motion` nothing moves: the ball rests on the net and the trail shows, so the
logo reads as the original.

## Topbar

The bar has no background or border of its own. `.topbar::before` (7 rem tall) carries a
`backdrop-filter: blur(20px)` masked with a top-to-bottom gradient, so content scrolling under it is
fully blurred at the top edge and clear at the bottom. **The header element itself must never get
`backdrop-filter`, `opacity < 1`, `transform` or `filter`**: any of them makes it a backdrop root and
the blur layer samples nothing.

The bar is a three-column grid (`1fr auto 1fr` from `md`), so the links are centred on the page whatever the
widths of the logo and the button. Below `md` the links are hidden and the grid is
`auto 1fr auto`, so the logo column is always as wide as the logo; with `1fr` columns a phone squeezes it. The page has no dark band, so the bar keeps one colour scheme everywhere.

- **Depth wash.** `site.js` sets `--tint` on the bar from 0 at the top of the page to 0.72 after
  1.5 viewport heights (once per animation frame on scroll). The blur layer's background is white at
  that alpha, under the same top-to-bottom mask, so the bar gains a white wash the deeper the page is
  read.
- **Scroll-spy.** The link whose section (`#the-place`, `#rules`, `#faqs`) crosses the middle band of
  the viewport gets `.is-active`: bold, with an underline that grows from the centre (blue). On the hero no link is active.

## Topbar on the hero

The topbar logo and its Book button (`.nav-reveal`) are hidden while the hero's logo (`#hero-logo`)
is on screen, and fade in once it scrolls under the nav (`site.js`, an IntersectionObserver with the
nav's 72 px as top margin). On the hero, the big logo and the hero's own Book button are the ones
that count; a second copy in the topbar reads as a duplicate. Hidden means `visibility: hidden`, so
the hidden button cannot be tabbed to.

## Court floor

The hero and the `/book` band have no grid; behind them is `.court-floor`: one inline SVG court
(44 × 20 ft as a 200 × 440 drawing, kitchen tinted green, dashed net across) tilted with
`rotateX(62deg)` so it reads as the floor seen from the baseline, faded by a radial mask. Its colour
is `currentColor`: `text-dink` in the hero, `text-dink` at lower opacity on the `/book` band.


## Net footer

The footer is drawn as the top of a net: `.net-footer::after` is the white tape along its top edge
(16 px, hairline edges, a soft shadow so it separates from the white page above), and
`.net-footer::before` is a 22 px square mesh of faint lines that fades out downward (mask). Both are
kept faint on purpose so the footer copy stays the thing that is read. The content sits in a
`relative` container so it paints above the mesh.

## Motion

`public/js/site.js` adds `.in` to each `.reveal` element as it scrolls into view (fade and rise,
700 ms). Under `prefers-reduced-motion` every `.reveal` is shown at once and smooth scrolling is off.

## Component classes

`btn`, `btn-primary`, `btn-ghost`, `btn-danger`, `card`, `icon-box`, `eyebrow`, `pill` and `grid-paper` (the map panel) are
Tailwind v4 `@utility` definitions. They must stay `@utility`, not `@layer components` classes:
Tailwind v4 refuses `@apply` of a plain class, and `btn-primary` applies `btn`.

## Confirmations

No page uses the browser's `confirm`, `alert` or `prompt`. `site.js` (loaded on every page) defines
`ask(text, { ok, cancel, danger })`, which opens a styled `<dialog class="ask">` and resolves to
`true` or `false`; Escape and a click on the backdrop answer `false`. A destructive action passes
`danger: true` for the red `btn-danger` button.

## Logo files

- `public/img/logo.png`: the flat wordmark, 1218 × 230, transparent (Open Graph image).
- `public/img/logo-base.png`, `logo-trail.png`, `logo-paddle-left.png`, `logo-paddle-right.png`,
  `logo-ball.png`: the playing logo's layers (§ Playing logo).
- `public/img/favicon.png`: the ball, 96 × 96, circular, transparent.

The layers are cut from the owner's logo image (on white). The background is turned to alpha by
un-blending from white, so edges sit cleanly on any background. Under a paddle the letter is redrawn:
a straight stem from the stem's own row colour and edge profile, and a round bowl from a circle
fitted to its visible edge. Paddle edges are un-blended from that redrawn letter, so a swinging paddle
leaves no halo.
