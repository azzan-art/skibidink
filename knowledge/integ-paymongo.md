# PayMongo: paying online

InstaPay is reached through a payment gateway; there is no InstaPay API for a merchant. PayMongo's
hosted checkout takes QR Ph (the InstaPay QR every Philippine bank and e-wallet app scans), and
GCash, Maya and cards when the account has them. `server-rs/src/pay.rs`.

## On or off

Online payment is on only when **all three** are set: `PRICE_PER_HOUR` (whole pesos),
`PAYMONGO_SECRET_KEY`, `PAYMONGO_WEBHOOK_SECRET`. Otherwise `/book` offers only "Arrange with us".
The price is the owner's: never fill it in.

`PAYMONGO_METHODS` (default `qrph`) is the comma list sent as `payment_method_types`. A method the
account has not enabled makes PayMongo refuse the session; the request is then kept as a plain
`requested` one and the visitor is told payment is down.

## Checkout

`POST https://api.paymongo.com/v2/checkout_sessions`, HTTP Basic with the secret key and an empty
password. One checkout per request, one line item per block: `amount` = price × hours × 100 (centavos), `PHP`.
`reference_number` is `SD-<bundle>`; the session id (`cs_…`) is saved in `checkout_id` on every
row of the bundle. Success returns
to `SITE_URL/book?paid=<id>`, cancel to `/book?unpaid=<id>`; neither return proves anything,
only the webhook does.

## Webhook

Register `https://<host>/api/pay/paymongo` in the PayMongo dashboard for
`checkout_session.payment.paid`, and put that webhook's secret in `PAYMONGO_WEBHOOK_SECRET` (it is
not the API key).

- `Paymongo-Signature: t=<unix>,te=<test sig>,li=<live sig>`. The signature is HMAC-SHA256, hex, of
  `"{t}.{raw body}"` with the webhook secret. It is checked against the raw bytes before any JSON
  parsing; whichever of `te`/`li` is non-empty must match. A bad signature is 401 and touches
  nothing.
- The rows are found by `data.attributes.data.id` (`checkout_id`), else by
  `data.attributes.data.attributes.reference_number` against `SD-<bundle or id>`. Each row then
  goes through `book::confirm` with `paid` on its own (`arch-booking.md`), so one block can be
  `Taken` while the others confirm.
- Every handled event answers 200 with JSON; anything else makes PayMongo retry.
- The route has no `Sec-Fetch-Site` or `Origin` header, which the cross-site write guard lets
  through (`arch-server.md`).

Hex output is inferred from PayMongo's samples; check it against the first real delivery in test
mode.
