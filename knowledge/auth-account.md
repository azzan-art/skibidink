# Customer accounts

`server-rs/src/account.rs`, `public/account.html`, `public/js/account.js`. Only a signed-in account
with a confirmed email can send a booking request (`arch-booking.md`); the request takes the name,
phone and Facebook from the account, never from the form.

## Registering

1. `/account` → Create account: name, email, password (8–128 characters), phone (required: the
   owner arranges payment through it), Facebook (optional).
2. The server stores the account unverified and emails a confirmation link
   (`/api/account/verify?token=…`, 48 hours, `integ-mail.md`).
3. The link marks the email verified, signs the visitor in and lands on `/account?verified=1`.

**The email's domain must take mail.** Before anything is stored, `domain_takes_mail` asks Google's
DNS-over-HTTPS (`dns.google/resolve`, 3 s) for the domain's MX, then its A record. When neither
answers (NXDOMAIN, or SERVFAIL from a domain whose nameservers do not respond), register answers
400 "Please use a Gmail account." Brevo resolves the same way, so such an address only soft-bounces
after the visitor has been told to check their email. The check fails open: when `dns.google`
itself cannot be reached, the address is accepted, so a Google outage never stops sign-ups. `.ph`
answers every name with a wildcard A record, so a misspelt `.ph` domain passes; test with
`.invalid`.

**Register answers the same whether or not the email is taken**, so the form cannot reveal who has
an account. A taken, verified address gets an email saying it already has an account; a taken,
unverified one is overwritten with the new details and gets a fresh link (nobody has proved they
own it yet).

Signing in an unverified account answers 403 with `unverified`; the page then offers to send the
link again.

## Passwords

Argon2id (`argon2` crate, default parameters), salt from `getrandom`, stored as the PHC string.
Hashing and checking run in `spawn_blocking`: each takes tens of milliseconds and ~19 MB, and on an
async worker it stalls every other request. An unknown email is checked against a decoy hash so the
answer takes the same time.

## Forgot password

`/api/account/forgot` always answers success; an existing account gets `/account?reset=<token>`,
valid 1 hour. The email is looked up in `users` first: an unknown address never reaches the mailer
(no Brevo, Resend or SMTP call), and the answer is the same either way so the form cannot be used to
test which emails have accounts. A reset sets the password, marks the email verified (the link proved it), deletes
every session of the account and signs this browser in.

## Sessions and tokens

- `user_sessions`: the SHA-256 of a 32-byte random token, the user, an expiry 30 days out. The raw
  token lives only in the cookie, so a database leak hands out no sessions.
- Cookie `sd_user`: `HttpOnly`, `SameSite=Lax` (so arriving from an email link keeps the session),
  `Path=/`, `Secure` in production.
- `email_tokens`: hashed the same way, with `kind` `verify` or `reset`. A new token of a kind
  retires the older unused ones of that kind; a token is used once.
- The sweeper deletes expired sessions and day-old expired tokens once an hour.
- `App::account_buckets` limits register, login, resend, forgot and reset to 10 at once per
  address, refilling 5 a minute.

## Links

Every emailed link starts with `SITE_URL`, never the request's `Host`: a forged `Host` would
otherwise send reset links to someone else's domain. `SITE_URL` must therefore be the origin the
Rust server itself serves (locally `http://localhost:3917`).

## Signed-out visitors

While the server has a database, a visitor without a verified session is a guest (`site::is_guest`;
a failing session lookup also counts as a guest, so a database hiccup never takes the homepage
down):

- `/book` answers 302 to `/account?next=%2Fbook`.
- The homepage is served as its guest variant, cached under its own key: every
  `href="/book" data-guest-href="X"` becomes `href="X"`, and every `<span data-guest="X">…</span>`
  becomes `<span>X</span>` (`site::for_guest`). A new Book link needs both attributes, or a guest
  sees a Book button that bounces them to sign-in.

The swap happens on the server so a signed-in visitor never sees the guest text flash first. With
no database (GitHub Pages, a local run without `DATABASE_URL`) nobody is a guest and the pages stay
as written, with the board in preview.

## The page

`/account` is `noindex` and not in the sitemap; `bin/static` builds it anyway for the demo
(`arch-demo.md`). Views: sign in / create account / forgot, choose a new password, "check your
email", and signed in. Signed in is the customer's home: "Booked" (confirmed, today on), "Requests"
(requested or awaiting payment, today on), "Past and closed" (folded), and "Account settings"
(name, phone, Facebook). A request whose hours someone else confirmed first shows "Taken" with a
link back to the board (`arch-booking.md` § Taken requests). `?next=/book` returns there after
sign-in; only a same-site path is followed.

## Phone fields

Every `input[name=phone]` is formatted as it is typed (`formatPhone` in `site.js`, loaded on every
page): a local number as `0917 123 4567` (4-3-4, at most 11 digits), `+63`/`63` as
`+63 917 123 4567`, any other `+` number keeps the spacing the person typed. The caret stays after
the same digit, and Backspace just after a space deletes the digit before it. The server accepts
the spaces (`book::parse_input`).

## Blocked accounts

The owner can block an account from the admin ("Block account" on any request that has one, with
an optional reason of up to 200 characters): `users.banned_at` and `users.ban_reason`. A blocked
account still signs in; `POST /api/reservations` answers 403 with `banned: true`, `/api/account/me`
reports `banned` and `banReason`, and the account home shows a red notice with the reason in place
of the Book button. Its open requests stay as they are for the owner to decline. "Unblock account"
clears both columns.

## Demo account

With `DEMO_EMAIL` and `DEMO_PASSWORD` set and `NODE_ENV` not `production`, boot creates (or
resets) a verified customer, "Demo Player", right after the schema. The password is set back to
`DEMO_PASSWORD` on every boot. It never runs in production: a password kept in a file there is a
way in. The local values are in `.env`.
