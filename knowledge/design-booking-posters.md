# Booking posters

`server-rs/src/poster.rs` draws a 1080 × 1350 PNG (Facebook's 4:5 portrait) for posting the schedule:
an SVG built as a string, rendered by `resvg`. Posters show times only, never names or contacts.

| Poster | Shows |
| --- | --- |
| day | Every hour of the day as a card: BOOKED (`dink`), OPEN (`skibi` label) or PAST (faded); the subtitle counts open hours or says "Fully booked" |
| week | Titled "This week" when it holds today, else "Week of Nov 28". Seven columns from the start date, one cell per hour; a FULL badge on a day with no open hour; counts fully booked days |
| month | The calendar month from Monday: FULL days filled `dink`, days with some bookings say "N open", untouched days "Open", past days faded |

Common frame: `mist` background, a `skibi` strip on top, `public/img/logo.png` (read per request and
embedded as a data URI), an eyebrow, title and subtitle, the `SITE_URL` host bottom left and
"Message us to book…" bottom right. No phone or address until the owner supplies one.

## Fonts

The fonts are compiled into the binary (`include_bytes!` from `server-rs/fonts/`): Outfit 700/800
for titles and numbers, Plus Jakarta Sans 500/700 for labels, the site's two families. Nothing is
read from the system, so the container needs no font packages.

The static Outfit files name their family **"Outfit Thin"** (they are cut from the variable font),
so the SVG asks for `DISPLAY = "Outfit Thin"`. A family resvg cannot match is dropped silently:
the text simply is not drawn.

Rendering runs in `spawn_blocking`. `cargo test poster` renders all three; with `POSTER_OUT=<dir>`
it also writes them there to look at.
