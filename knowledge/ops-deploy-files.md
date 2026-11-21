# Which files git keeps, which ones the container gets

| Where | Answers |
| --- | --- |
| `.gitignore` | what is not part of the project at all |
| the last stage of `render/Dockerfile` | what the running container holds |
| `.dockerignore` | what the build context leaves out, so secrets and caches never reach the builder |

## Git

Git tracks source, pages, `knowledge/` and `CLAUDE.md`. Out: secrets (`.env*` except
`.env.example`, keys), dependencies (`node_modules/`, `.bin/`), generated output
(`public/styles/output.css`, `server-rs/target/`, `dist/`, `router/dist/`, `.wrangler/`) and machine noise.
**Never remove a line from the secrets block**: a committed `.env` stays leaked after deletion.

## The container

Render builds `render/Dockerfile` from the repo (`render.yaml`, Docker runtime, health check
`/api/ready`). The last stage holds `public/`, the compiled `output.css` and the `skibidink` binary,
nothing else. The server stage also copies `server-rs/fonts/`, which the poster code compiles in
with `include_bytes!`; without it the build fails. **A folder the server reads at runtime must be copied there**; a missing one does not
fail the build, it fails the first request that reaches for it. `TZ=Asia/Manila` is set for every
date the server prints.

`render.yaml` `buildFilter.ignoredPaths` skips a rebuild for changes only to `knowledge/`,
`router/`, Markdown and `.claude/`.

## Environment

| Variable | Where | Purpose |
| --- | --- | --- |
| `PORT` | platform | Port to bind (default 3000) |
| `NODE_ENV=production` | image | Turns on HSTS, the page cache and `/_v/` asset URLs |
| `DATABASE_URL` | Render | Postgres; optional. Supabase's session pooler (port 5432), never the transaction pooler |
| `DB_SCHEMA` | optional | The schema the app's tables live in, default `skibidink` (`arch-server.md` § Boot) |
| `EDGE_SECRET` | Render + Worker secret | Same value on both sides (`ops-edge.md`) |
| `SITE_URL` | Render | Canonical origin (`arch-server.md` § Page composition), and the origin of every emailed link (`auth-account.md`) |
| `RATE_LIMIT` | optional | Points per second per address |
| `RENDER_GIT_COMMIT` | Render | The `?v=` version for unhashed assets |
| `ADMIN_PASSWORD` | Render | The admin sign-in (`auth-admin.md`); unset closes the admin |
| `PRICE_PER_HOUR` | Render | Whole pesos per court hour, the owner's figure; unset keeps online payment off |
| `PAYMONGO_SECRET_KEY` | Render | PayMongo API key (`integ-paymongo.md`) |
| `PAYMONGO_WEBHOOK_SECRET` | Render | The webhook endpoint's secret |
| `PAYMONGO_METHODS` | optional | Checkout methods, default `qrph` |
| `MAIL_PROVIDER` | optional | `brevo` or `log` (`integ-mail.md`) |
| `BREVO_API_KEY` | Render | Brevo API key |
| `MAIL_FROM` | Render | Sender address on a domain verified in Brevo |
| `MAIL_FROM_NAME` | optional | Sender name, default "SkibiDink" |
