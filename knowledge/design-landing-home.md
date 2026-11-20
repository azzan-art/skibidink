# The homepage

`public/index.html`, composed by `site::home`. The layout follows a clean sports-venue landing
page: a frosted fixed nav, a hero over a faded court, and alternating light sections with cards.
Colours come from the SkibiDink logo.

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
| `skibi` | `#1f9d48` | Eyebrows, pills (the logo's "Skibi" green) |
| `ball` | `#a3e635` | The ball in the court illustration |
| `ink` / `body` / `muted` | slate 900 / 800 / 500 | Headings / text / secondary text |
| `paper` / `line` | `#f8fafc` / `#e2e8f0` | Page background / borders |

## Sections

| id | Content |
| --- | --- |
| hero | The playing logo as the `<h1>` (with an `sr-only` text name, § Playing logo), tagline, two buttons, over the court floor (§ Court floor) |
| `the-place` | The place and its location in one: a gallery on the left (two photo frames over a wide map panel) and the copy plus four cards (hours, open play, address, contact) on the right |
| `book` | The live booking board (§ Court board), headed "Book", on a dark band |
| `rules` | Four house-rule cards |
| `faqs` | `<details>` accordion, no script |
| `join` | Call-to-action banner pointing back to `#book` |

**The photos, address, contact, map and real booking are placeholders.** The two `.photo-slot` frames take an `<img>` (object-cover) when the owner sends photos; `.map-slot` takes a Google Maps embed `<iframe>` once the location is known. They state nothing about a real address or
phone until the owner supplies them; the structured data stays in step (`arch-server.md`).

## Court board

`public/js/book.js` draws the venue's one court on a floor plan (lounge, paddle rental, restrooms,
entrance, benches, water station) and runs a **simulated** schedule of hourly slots, 6 AM to 10 PM. Nothing is sent to the server; the board
is labelled a preview.

- **Every visitor sees the same board.** Whether a slot is booked comes from a hash of
  `date|court|hour`, never `Math.random`, and the page reads Manila time (`Asia/Manila`), not the
  visitor's clock zone. A random source would show each visitor a different venue.
- Each booked slot also has a seeded "booked at" moment up to 48 hours before it starts. A slot
  shows open until then, so the board fills as the day goes on and the feed lists the bookings made
  in the last 12 hours, the same list for everyone at the same minute.
- Demand rises at 5–9 PM, 6–8 AM and weekends and dips at midday (`demand()`).
- The board's clock shows Manila time as `1:50 PM`.
- A court booked for the current hour is "in play": players and a ball animate on it. The stats
  show the court's state right now, the next open hour today and the open slots left today.
- "Hold this slot" stores the pick in `localStorage` (`skibidink.held`); only that visitor sees it.
- `POS` lists the courts. One court is 72 % × 47.6 % of the floor, which keeps the 44 × 20 ft
  ratio at the floor's 16:11 aspect. With one entry the board says "The court" and hides court
  names in the feed; adding entries brings back per-court names and the selection outline. The kitchen spans 34.1 %–65.9 % of the court's length (7 ft each side of the net).
- The published page is the same file for everyone; all of this runs in the browser after load,
  which is why the board works on static hosting (`ops-pages.md`).

When real booking exists, the seeded schedule is replaced by the server's, and the hold becomes a
request to it.

## Playing logo

The hero logo is four stacked layers in `.logo-play`, all on the same 778 × 135 canvas so they line
up with no offsets: `logo-base.png` (the wordmark without paddles or ball, the "i" stems and the
"b" bowl filled back in), `logo-paddle-left.png`, `logo-paddle-right.png`, and `logo-ball.png`.

The two "i"s rally on one 11 s loop, all five animations sharing it:

| Time | Ball | Paddles |
| --- | --- | --- |
| 9.5 s → 3 s (wraps) | **At rest in the "b"**, rotation a whole number of turns | Still |
| 3 → 4 s | Hops out to the right paddle | Right winds up and hits at 4 s |
| 4 → 7 s | Arcs slowly over the net to the left paddle | Left winds up and hits at 7 s |
| 7 → 9.5 s | Arcs over the net and drops back into the "b" | Still |

The rest phase is what keeps the logo legible: for 4.5 s of every loop it is exactly the original
mark. **The ball's rest position (`left: 37.16%`, `top: 40.7%`) is its place in the original logo
and doubles as the reduced-motion position**; the spin keyframes end on 720° so the ball's holes
land where they were. The keyframe percentages, the swing contact points (36.36 %, 63.64 %) and the
11 s duration are tuned together; changing one alone puts the hits out of step with the ball.

`transform-origin` on each paddle is its hand in canvas percent (left 29.05 % 29.6 %, right
51.03 % 29.6 %), and the ball's `left`/`top` keyframes are canvas percent of the paddle faces and
the arc's peak above the net. Re-cutting the layers from a new logo means re-measuring both.

Under `prefers-reduced-motion` nothing moves and the ball rests in the bowl, so the logo reads as
the original. The nav and footer use the flat `logo.png`.

## Topbar

The bar has no background or border of its own. `.topbar::before` (7 rem tall) carries a
`backdrop-filter: blur(20px)` masked with a top-to-bottom gradient, so content scrolling under it is
fully blurred at the top edge and clear at the bottom. **The header element itself must never get
`backdrop-filter`, `opacity < 1`, `transform` or `filter`**: any of them makes it a backdrop root and
the blur layer samples nothing.

The bar is a three-column grid (`1fr auto 1fr`), so the links are centred on the page whatever the
widths of the logo and the button. Over a section marked `data-dark` (the `book` band) the links turn
light (`.on-dark`, set by an IntersectionObserver watching the top 8 % of the viewport).

- **Depth wash.** `site.js` sets `--tint` on the bar from 0 at the top of the page to 0.72 after
  1.5 viewport heights (once per animation frame on scroll). The blur layer's background is white at
  that alpha, under the same top-to-bottom mask, so the bar gains a white wash the deeper the page is
  read. Over a `data-dark` band the wash is navy instead, so it never lays a white haze on the dark
  section.
- **Scroll-spy.** The link whose section (`#the-place`, `#rules`, `#faqs`) crosses the middle band of
  the viewport gets `.is-active`: bold, with an underline that grows from the centre (blue, lime over
  the dark band). On the hero and the sections with no link (`#book`, `#join`) no link is active.

## Topbar on the hero

The topbar logo and its Book button (`.nav-reveal`) are hidden while the hero's logo (`#hero-logo`)
is on screen, and fade in once it scrolls under the nav (`site.js`, an IntersectionObserver with the
nav's 72 px as top margin). On the hero, the big logo and the hero's own Book button are the ones
that count; a second copy in the topbar reads as a duplicate. Hidden means `visibility: hidden`, so
the hidden button cannot be tabbed to.

## Court floor

The hero and the `book` band have no grid; behind them is `.court-floor`: one inline SVG court
(44 × 20 ft as a 200 × 440 drawing, kitchen tinted green, dashed net across) tilted with
`rotateX(62deg)` so it reads as the floor seen from the baseline, faded by a radial mask. Its colour
is `currentColor`: `text-dink` in the hero, `text-ball` (lower opacity) on the dark band.


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

`btn`, `btn-primary`, `btn-ghost`, `card`, `icon-box`, `eyebrow`, `pill` and `grid-paper` (the map panel) are
Tailwind v4 `@utility` definitions. They must stay `@utility`, not `@layer components` classes:
Tailwind v4 refuses `@apply` of a plain class, and `btn-primary` applies `btn`.

## Logo files

- `public/img/logo.png` — the flat wordmark, 778 × 135, transparent (nav, footer, Open Graph).
  White background and letter counters are removed; the white inside the net is kept.
- `public/img/logo-base.png`, `logo-paddle-left.png`, `logo-paddle-right.png`, `logo-ball.png` —
  the playing logo's layers, cut from `logo.png` (§ Playing logo).
- `public/img/favicon.png` — the ball, 96 × 96, circular, transparent.

Both are cut from the owner's logo image by colour keying: near-white regions become transparent
except inside the net's column band, and pixels at the cut edge are un-blended from white so they
sit cleanly on any background.
