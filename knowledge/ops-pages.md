# Publishing on GitHub Pages

The public site is GitHub Pages, served from `azzan-art/skibidink` at
`https://azzan-art.github.io/skibidink/`. Pages serves files only, so the Rust server renders the
pages once at build time and the files are published; nothing runs per request.

## The build

`.github/workflows/pages.yml` runs on every push to `main` (and by hand):

1. installs Rust, caches `server-rs/target`;
2. `actions/configure-pages` reports the site's `base_path` (`/skibidink`, or empty on a custom
   domain) and `base_url`;
3. `bin/static` builds `dist/`;
4. `upload-pages-artifact` and `deploy-pages` publish it.

## `bin/static`

1. Compiles `output.css` (minified) and the release server.
2. Runs the server with `env -i`, no `DATABASE_URL`, `SITE_URL` set to the Pages URL and
   `RENDER_GIT_COMMIT` set to the commit, so every asset URL carries `?v=<commit>`. It runs outside
   production mode on purpose: no `/_v/<hash>/` URLs, which plain file hosting does not have.
3. Copies `public/` without `.html` into `dist/`, and saves every `<loc>` in `public/sitemap.xml`
   as `dist/<path>/index.html`.
4. With `BASE_PATH` set, rewrites every root-relative `href="/…"` and `src="/…"` in the HTML and
   every `url(/…)` in the CSS to start with it. **A new root-relative reference in another form
   (a path built in JavaScript, `srcset`, an inline style) is not rewritten** and 404s under the
   project path.
5. Points `robots.txt` and `sitemap.xml` at `SITE_URL`, and writes `.nojekyll` so Pages serves
   every file as-is.

Run it locally the same way: `BASE_PATH=/skibidink SITE_URL=https://azzan-art.github.io/skibidink
bin/static`, then serve the parent of a folder named `skibidink` that holds `dist/`.

## Consequences of static hosting

- **Only sitemap pages exist.** A page not listed in `public/sitemap.xml` is never built.
- **No API.** `/api/*` routes, the database and anything per-visitor do not exist on Pages. The
  court board works because it runs entirely in the browser (`design-landing-home.md` § Court
  board). Real bookings need the Rust server hosted (`render.yaml`) and the board pointed at it.
- Pages sends its own caching headers (about 10 minutes); `?v=<commit>` is what makes a new deploy
  reach returning visitors.
- The repository is public, which Pages on a free plan requires.

The Cloudflare router (`ops-edge.md`) is not deployed while Pages is the host.
