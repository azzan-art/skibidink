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
| `the-place` | CSS-drawn court (no photo yet) and the facility copy with two feature cards |
| `book` | The live booking board (§ Court board), headed "Book", on a dark band |
| `rules` | Four house-rule cards |
| `location` | Contact list and a browser-frame map panel; both say "coming soon" |
| `faqs` | `<details>` accordion, no script |
| `join` | Call-to-action banner pointing back to `#book` |

**Location, contact and real booking are placeholders.** They state nothing about a real address or
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

The two "i"s rally. On one 6 s cycle the ball crosses left to right in 3 s and back (`ball-x`,
linear), arcing over the net twice per cycle (`ball-y`, slowing as it rises), and spinning. Each
paddle (sleeve, grip and head only) swings from its hand when the ball reaches it: left at 0 s,
right at 3 s. **The swing keyframes, the ball durations and the 6 s cycle are tuned together**;
changing one alone puts the hits out of step with the ball.

`transform-origin` on each paddle is its hand in canvas percent (left 29.05 % 29.6 %, right
51.03 % 29.6 %), and the ball's `left`/`top` keyframes are canvas percent of the paddle faces and
the arc's peak above the net. Re-cutting the layers from a new logo means re-measuring both.

Under `prefers-reduced-motion` nothing moves and the ball rests in the bowl, so the logo reads as
the original. The nav and footer use the flat `logo.png`.

## Nav logo

The topbar logo (`#nav-logo`) is hidden while the hero's logo (`#hero-logo`) is on screen, and fades
in once it scrolls under the nav (`site.js`, an IntersectionObserver with the nav's 72 px as top
margin). Two logos on one screen read as a duplicate.

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
