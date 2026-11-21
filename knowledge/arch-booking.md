# Booking: requests, payment, confirmation

`server-rs/src/book.rs` (public side, the rules) and `admin.rs` (the owner's side). One court, hourly
slots from `OPEN` 6 to `CLOSE` 23 (the last slot is 10–11 PM), Manila time (`date::manila_now`,
UTC+8, no daylight saving).

## How a slot gets booked

The venue's practice: a visitor sends a request, payment is settled, and only then is the slot
theirs. Several people may request the same hours; the first to pay wins.

| status | Meaning | Blocks the slot? |
| --- | --- | --- |
| `requested` | Sent from `/book` with "Arrange with us", or reopened by the owner | no |
| `awaiting` | Payment is arranged; `pay_by` is the deadline. Online-pay requests start here with 60 minutes | no |
| `confirmed` | Paid (or the owner chose "Confirm, pay at court") | **yes** |
| `declined` / `cancelled` | Closed by the owner | no |

- **Only `confirmed` blocks.** The public board, the calendar, the posters and the
  overlap checks read `confirmed` rows only. A request for hours already confirmed is refused (409);
  a request that overlaps other open requests is accepted.
- **`book::confirm` is the only way to `confirmed`.** It takes a per-day advisory lock
  (`pg_advisory_xact_lock(hashtext('reservations:' || day))`), re-checks the overlap inside the
  transaction, then updates. Writing `status = 'confirmed'` anywhere else lets two payments land on
  the same hour. Answers `Done`, `Taken` (another confirmed booking overlaps) or `Missing`.
- A payment that arrives for hours already `Taken` still records `paid_at` and leaves the status as
  it was; the admin list shows it in red as "paid online too: refund or move it".
- `pay_by` is shown, never enforced: an overdue `awaiting` request stays until the owner declines it.

## One request, several hours

On `/book` each open hour is a toggle; the visitor picks up to 8 hours (`MAX_PICKED_HOURS`), across
any of the 7 days on the board, and sends them as one request: `slots: [{date, hour}]`.
`book::picked_blocks` sorts and de-duplicates them and joins back-to-back hours on one day into a
block (at most `MAX_HOURS`, 4, per block). Each block is one `reservations` row, validated by
`parse_input` like any other; all rows of one request share `bundle`, the id of the first row,
which is the request number the visitor sees. If any block overlaps a confirmed booking, the whole
request is refused (409) and nothing is saved. With online pay the bundle gets one checkout
(`integ-paymongo.md`); the owner still decides each row on its own in the admin.

A day tab with no open hour left (all confirmed, or today after the last slot has started) is
disabled and marked "Full", and the board moves off it.

## Taken requests

When `book::confirm` confirms a row, the same transaction stamps `taken_at` on every `requested` or
`awaiting` row it overlaps that has none yet. After the commit, each account among them gets one
email listing its taken hours (`book::tell_taken` → `account::send_taken`, sent in a spawned task so
a slow mailer never delays the confirmation); walk-ins have no account and get nothing. The account
home shows such a row as "Taken", and `confirm` answers `Done(n)` so the admin toast says how many
players were emailed. Cancelling, declining or reopening any row runs `book::reopen_taken` for its
day, which clears `taken_at` on rows no confirmed booking overlaps any more; they are not emailed
again. The status itself is never changed: the owner still declines a taken request.

## Competing requests

Every row in the admin list carries `competing`: the other `requested`, `awaiting` and `confirmed`
rows on the same day whose hours overlap. The list shows them with their phone links and Facebook
so the owner can message each one ("someone else wants this slot, pay now").

## Who can request

`POST /api/reservations` needs a signed-in account with a confirmed email (`auth-account.md`);
without one it answers 401 with `signIn: true` and the board sends the visitor to
`/account?next=/book`. The name, phone and Facebook come from the account and the row keeps its
`user_id`. Walk-ins from the admin have no account.

## Contact and validation

`book::parse_input` (shared by the public form and walk-ins): name 2–80 characters; a phone
(digits, spaces, `+-()`, at most 30) or a Facebook name/link (at most 200), at least one of the
two; 1–16 players; 1–4 hours (walk-ins up to the whole day); inside opening hours. The public form
also needs a day from today to 60 days ahead and an hour that has not started. Walk-ins may be any
day, so the owner can record a past booking.

`POST /api/reservations` is limited per address by `App::request_buckets` (5 at once, refilling one
every 5 minutes) on top of the global rate limit, and keeps the sender's address in `ip`.

## Table

`reservations` (created by `book::SCHEMA` on every boot, `IF NOT EXISTS`): `day` date,
`start_hour`/`hours`/`players` smallint, `name`, `phone`, `facebook`, `note`, `status`,
`pay_method` (`arrange`, `online`, `walkin`), `pay_by`, `paid_at`, `amount` (centavos, online only),
`checkout_id` (PayMongo session), `bundle` (the first row id of a multi-hour request), `taken_at`
(another booking took its hours), `user_id` (the account, null for walk-ins), `decided_at`,
`created_at`. Schema changes must stay
`IF NOT EXISTS` / `ADD COLUMN IF NOT EXISTS`, because the batch runs on every boot.

## Routes

| Route | Who | Does |
| --- | --- | --- |
| `GET /api/availability?from&days` | anyone | Confirmed hours per day (default 7 days, at most 62), opening hours, whether online pay is on and the hourly price |
| `POST /api/reservations` | anyone | A request for `slots`; answers `id` (the bundle) and `ids` (one per block), and with `pay: "online"` also opens a checkout and answers `checkoutUrl` |
| `POST /api/pay/paymongo` | PayMongo | The payment webhook (`integ-paymongo.md`) |
| `GET /admin` | anyone | The admin page; everything on it needs a session (`auth-admin.md`) |
| `GET /api/admin/reservations?scope&q` | admin | The list: `upcoming` (default), `past` or `all`, always in date and hour order; `q` searches name, phone, Facebook, note, date and `#id`; at most 500 rows (`past` keeps the 500 most recent) |
| `POST /api/admin/reservations` | admin | A walk-in: saved straight to `confirmed` through `confirm`, deleted again if the hours are taken |
| `POST /api/admin/reservations/{id}` | admin | `{status, payByHours, paid}`: change status, set the pay-by deadline, mark paid or unpaid; a confirm answers `taken` |
| `POST /api/admin/users/{id}/ban` | admin | `{ban, reason}`: block or unblock an account (`auth-account.md` § Blocked accounts) |
| `GET /api/admin/calendar?from&to` | admin | Confirmed rows in a range of at most 62 days |
| `GET /api/admin/stats` | admin | The dashboard numbers |
| `GET /api/admin/poster/{day,week,month}` | admin | A PNG (`design-booking-posters.md`); `&download=1` makes it an attachment |

## The admin page

`public/admin.html` + `public/js/admin.js`, three tabs:

- **Requests**: every request in date and hour order, grouped by day, each with the account's
  email when it has one, a "Blocked" tag and a Block/Unblock button for accounts, filter chips (All,
  the default; Needs action, which is every requested or awaiting-payment one; Confirmed; Declined)
  counted on the client, search, the walk-in form, and the actions each status allows. "Awaiting payment" sets a
  24-hour pay-by; there is no action to move it later.
- **Calendar**: day, week and month views of confirmed bookings, a day in the month or week opens
  its day view, and "Download picture" fetches the poster for the view on screen.
- **Dashboard**: "Players so far" sums `players` of confirmed bookings up to today, the people who
  have actually tried the court. Customers are counted by account, else (walk-ins) by phone digits,
  else the Facebook text, else the name, so one person booking twice counts once and "returning" is anyone with more than one
  confirmed booking. Bars: players per month (6 months), bookings per hour of day, per weekday.

The admin page is not in the sitemap and its `robots` is `noindex, nofollow`; `bin/static` builds
it anyway for the demo (`arch-demo.md`).
