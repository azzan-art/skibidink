# Sending email

`server-rs/src/mail.rs`. Every email goes through `Mailer::send(http, Email { to, subject, text,
html })`; nothing else in the server knows which provider is behind it.

| `MAIL_PROVIDER` | Needs | Does |
| --- | --- | --- |
| `brevo` (default when `BREVO_API_KEY` is set) | `BREVO_API_KEY`, `MAIL_FROM` | `POST https://api.brevo.com/v3/smtp/email` with the `api-key` header |
| `log` (default otherwise) | nothing | Prints the email, link included, to the server log |

`MAIL_FROM_NAME` defaults to "SkibiDink". Boot prints which mailer is in use. A `brevo` setting with
a missing variable falls back to `log` with a warning.

**`log` in production prints working sign-in and reset links into the logs.** It is for a local
run; production must have a real provider.

`MAIL_FROM` must be an address on a domain verified in the provider (SPF/DKIM). A free-mail
sender such as `@gmail.com` fails the receiver's DMARC check and lands in spam or bounces.

The account emails share one centered, table-based layout (`email_html` in `account.rs`): tables
and inline styles, because Gmail and Outlook drop `<style>` blocks and flexbox. The logo is
`EMAIL_LOGO`, the PNG on the GitHub Pages site, not a `SITE_URL` path: a local run's `SITE_URL` is
`localhost`, which a mail client cannot fetch. Moving or renaming `public/img/logo.png`, or moving
the Pages site, breaks the logo in every email already sent and every one sent after.

Adding a provider: a new `Mailer` variant, its arm in `from_env`, `name` and `send`. The callers
stay as they are.

Brevo's free plan sends 300 emails a day; the site sends one per registration, one per password
reset, and one per account whose request a confirmed booking took (`arch-booking.md` § Taken
requests).
