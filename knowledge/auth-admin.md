# Admin sign-in

`server-rs/src/admin.rs`. One shared password, `ADMIN_PASSWORD` (at least 8 characters). Unset or
shorter, sign-in answers 503 and the admin is closed.

- The password is compared in constant time, and `App::login_buckets` allows 5 tries at once per
  address, refilling two a minute.
- A session is 32 random bytes (hex) kept **in memory** for 12 hours. A restart or deploy signs
  everyone out; that is the price of having no session table.
- The cookie is `sd_admin`: `HttpOnly`, `SameSite=Strict`, `Path=/`, and `Secure` in production.
  The cross-site write guard (`arch-server.md`) covers CSRF for the admin's POST routes.
- Every `/api/admin/*` route except `login` checks the session first (`guard!`) and answers 401
  without it; the page then shows the sign-in card.
