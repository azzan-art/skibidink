(() => {
  const script = document.currentScript
  const BASE = new URL(script.src).pathname.replace(/\/js\/demo\.js$/, '')
  window.SITE_BASE = BASE
  const KEY = 'skibidink.demo.v1'
  const OPEN = 6
  const CLOSE = 23
  const MAX_BLOCK = 4
  const MAX_PICKS = 8
  const realFetch = window.fetch.bind(window)

  const manila = new Intl.DateTimeFormat('en-CA', { timeZone: 'Asia/Manila', year: 'numeric', month: '2-digit', day: '2-digit', hour: '2-digit', hourCycle: 'h23' })
  function now() {
    const p = Object.fromEntries(manila.formatToParts(new Date()).map(x => [x.type, x.value]))
    return { today: `${p.year}-${p.month}-${p.day}`, hour: +p.hour }
  }
  const addDays = (iso, n) => { const d = new Date(`${iso}T12:00:00Z`); d.setUTCDate(d.getUTCDate() + n); return d.toISOString().slice(0, 10) }
  const daysBetween = (a, b) => Math.round((Date.parse(`${b}T12:00:00Z`) - Date.parse(`${a}T12:00:00Z`)) / 864e5)
  const fmtHour = h => `${h % 12 || 12} ${h < 12 || h === 24 ? 'AM' : 'PM'}`
  const longDate = iso => new Date(`${iso}T12:00:00Z`).toLocaleDateString('en-PH', { weekday: 'short', month: 'short', day: 'numeric', timeZone: 'UTC' })
  const overlaps = (a, b) => a.day === b.day && a.start_hour < b.start_hour + b.hours && b.start_hour < a.start_hour + a.hours
  const stamp = () => new Date().toISOString()

  function rng(seed) {
    return () => {
      seed = (seed + 0x6d2b79f5) | 0
      let t = Math.imul(seed ^ (seed >>> 15), 1 | seed)
      t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t
      return ((t ^ (t >>> 14)) >>> 0) / 4294967296
    }
  }

  const PEOPLE = [
    ['Ana Cruz', '0917 555 0101', 'ana.cruz'], ['Ben Reyes', '0918 555 0102', ''], ['Carla Santos', '0919 555 0103', 'carla.s'],
    ['Dan Lim', '0920 555 0104', ''], ['Ella Tan', '0921 555 0105', 'ella.tan'], ['Gio Ramos', '0922 555 0106', ''],
  ]

  function seed() {
    const { today, hour } = now()
    const r = rng(20261121)
    const users = [{ id: 1, email: 'demo@skibidink.test', name: 'Demo Player', phone: '0917 000 0000', facebook: '', banned_at: null, ban_reason: '' }]
    PEOPLE.forEach(([name, phone, facebook], i) => users.push({ id: i + 2, email: `${name.split(' ')[0].toLowerCase()}@example.com`, name, phone, facebook, banned_at: null, ban_reason: '' }))
    users[6].banned_at = stamp()
    users[6].ban_reason = 'Sent many requests and never paid'
    const rows = []
    let id = 1
    const add = (o) => {
      const u = users.find(x => x.id === o.user_id)
      const row = {
        id: id++, created_at: stamp(), name: u ? u.name : o.name, phone: u ? u.phone : o.phone || '', facebook: u ? u.facebook : '',
        day: o.day, start_hour: o.start_hour, hours: o.hours, players: o.players || 4, note: o.note || '', status: o.status,
        pay_method: o.pay_method || 'arrange', pay_by: o.pay_by || null, paid_at: o.status === 'confirmed' ? stamp() : null, amount: null,
        decided_at: o.status === 'confirmed' ? stamp() : null, user_id: o.user_id || null, taken_at: null, bundle: null,
      }
      row.bundle = row.id
      rows.push(row)
      return row
    }
    for (let d = -150; d < 0; d++) {
      const day = addDays(today, d)
      const n = Math.floor(r() * 4)
      const used = new Set()
      for (let k = 0; k < n; k++) {
        const start = OPEN + Math.floor(r() * (CLOSE - OPEN - 2))
        const hours = 1 + Math.floor(r() * 2)
        if ([...Array(hours)].some((_, j) => used.has(start + j))) continue
        for (let j = 0; j < hours; j++) used.add(start + j)
        const who = r() < 0.25 ? null : 2 + Math.floor(r() * 5)
        add({ day, start_hour: start, hours, players: 2 + Math.floor(r() * 5), status: 'confirmed', user_id: who, name: 'Walk-in guest', phone: '0927 555 0199', pay_method: who ? 'arrange' : 'walkin' })
      }
    }
    add({ day: addDays(today, -5), start_hour: 18, hours: 2, status: 'confirmed', user_id: 1 })
    for (let d = 0; d < 7; d++) {
      const day = addDays(today, d)
      if (d === 4) {
        add({ day, start_hour: OPEN, hours: CLOSE - OPEN, players: 16, status: 'confirmed', name: 'Open play night', phone: '0927 555 0199', pay_method: 'walkin', note: 'Club event, whole day' })
        continue
      }
      for (const [start, hours] of [[7, 1], [18, 2], [12, 1]]) {
        if (d === 0 && start <= hour) continue
        if (r() < 0.55) add({ day, start_hour: start + (d % 2), hours, status: 'confirmed', user_id: 2 + Math.floor(r() * 5) })
      }
    }
    add({ day: addDays(today, 2), start_hour: 9, hours: 2, status: 'confirmed', user_id: 1, note: 'Bringing my own paddles' })
    add({ day: addDays(today, 3), start_hour: 16, hours: 1, status: 'requested', user_id: 1 })
    add({ day: addDays(today, 3), start_hour: 16, hours: 2, status: 'awaiting', user_id: 2, pay_by: new Date(Date.now() + 20 * 3600e3).toISOString() })
    add({ day: addDays(today, 5), start_hour: 10, hours: 1, status: 'requested', user_id: 4 })
    const taken = add({ day: addDays(today, 6), start_hour: 20, hours: 1, status: 'requested', user_id: 1 })
    add({ day: addDays(today, 6), start_hour: 20, hours: 2, status: 'confirmed', user_id: 3 })
    taken.taken_at = stamp()
    return { users, rows, nextId: id, nextUser: users.length + 1, session: { user: null, admin: false }, outbox: [] }
  }

  let db
  try { db = JSON.parse(localStorage.getItem(KEY)) } catch {}
  if (!db || !db.rows) db = seed()
  const save = () => localStorage.setItem(KEY, JSON.stringify(db))
  save()

  const json = (status, body) => new Response(JSON.stringify(body), { status, headers: { 'Content-Type': 'application/json' } })
  const ok = body => json(200, { success: true, ...body })
  const fail = (status, error, extra = {}) => json(status, { success: false, error, ...extra })
  const me = () => db.users.find(u => u.id === db.session.user)
  const userOf = row => db.users.find(u => u.id === row.user_id)

  function mail(user, rows) {
    const slots = rows.map(r => `${longDate(r.day)}, ${fmtHour(r.start_hour)}–${fmtHour(r.start_hour + r.hours)}`)
    db.outbox.unshift({
      at: stamp(), to: user.email,
      subject: rows.length === 1 ? 'The hour you asked for was booked' : 'The hours you asked for were booked',
      text: `Hi ${user.name.split(' ')[0]}, someone else just booked and paid for ${rows.length === 1 ? 'this hour' : 'these hours'} you asked for: ${slots.join('; ')}. Pick another time on the booking page.`,
    })
  }

  function confirm(row, paid) {
    if (db.rows.some(o => o.id !== row.id && o.status === 'confirmed' && overlaps(o, row))) {
      if (paid) row.paid_at ||= stamp()
      return null
    }
    row.status = 'confirmed'
    row.decided_at = stamp()
    if (paid) row.paid_at ||= stamp()
    const bumped = db.rows.filter(o => o.id !== row.id && (o.status === 'requested' || o.status === 'awaiting') && !o.taken_at && overlaps(o, row))
    bumped.forEach(o => { o.taken_at = stamp() })
    const byUser = new Map()
    for (const o of bumped) {
      const u = userOf(o)
      if (u) byUser.set(u, [...(byUser.get(u) || []), o])
    }
    byUser.forEach((rows, u) => mail(u, rows))
    return bumped.length
  }

  function reopen(day) {
    for (const r of db.rows) {
      if (r.taken_at && r.day === day && !db.rows.some(c => c.status === 'confirmed' && overlaps(c, r))) r.taken_at = null
    }
  }

  function blocks(slots) {
    const picks = [...new Set(slots.map(s => `${s.date}|${String(s.hour).padStart(2, '0')}`))].sort()
    const out = []
    for (const k of picks) {
      const [date, h] = [k.slice(0, 10), +k.slice(11)]
      const last = out[out.length - 1]
      if (last && last.day === date && last.start_hour + last.hours === h && last.hours < MAX_BLOCK) last.hours++
      else out.push({ day: date, start_hour: h, hours: 1 })
    }
    return out
  }

  const publicRow = r => ({ id: r.id, day: r.day, start_hour: r.start_hour, hours: r.hours, players: r.players, status: r.status, pay_method: r.pay_method, pay_by: r.pay_by, paid_at: r.paid_at, taken_at: r.taken_at })
  const sortRows = (a, b) => a.day.localeCompare(b.day) || a.start_hour - b.start_hour || a.id - b.id

  function adminRow(r) {
    const u = userOf(r)
    return {
      ...r, email: u?.email ?? null, banned_at: u?.banned_at ?? null, ban_reason: u?.ban_reason ?? '',
      competing: db.rows.filter(o => o.id !== r.id && ['requested', 'awaiting', 'confirmed'].includes(o.status) && overlaps(o, r))
        .map(o => ({ id: o.id, name: o.name, phone: o.phone, facebook: o.facebook, status: o.status, start_hour: o.start_hour, hours: o.hours })),
    }
  }

  function stats() {
    const { today } = now()
    const c = db.rows.filter(r => r.status === 'confirmed')
    const who = r => r.user_id ? `u${r.user_id}` : r.phone.replace(/\D/g, '') || r.facebook.toLowerCase() || r.name.toLowerCase()
    const people = new Map()
    c.forEach(r => people.set(who(r), (people.get(who(r)) || 0) + 1))
    const sum = (rows, k) => rows.reduce((n, r) => n + r[k], 0)
    const months = []
    for (let m = 5; m >= 0; m--) {
      const d = new Date(`${today.slice(0, 7)}-01T12:00:00Z`)
      d.setUTCMonth(d.getUTCMonth() - m)
      const month = d.toISOString().slice(0, 7)
      const rows = c.filter(r => r.day.startsWith(month))
      months.push({ month, bookings: rows.length, players: sum(rows, 'players') })
    }
    const isoDow = iso => (new Date(`${iso}T12:00:00Z`).getUTCDay() + 6) % 7
    return {
      requests: db.rows.length,
      requested: db.rows.filter(r => r.status === 'requested').length,
      awaiting: db.rows.filter(r => r.status === 'awaiting').length,
      confirmed: c.length,
      declined: db.rows.filter(r => r.status === 'declined' || r.status === 'cancelled').length,
      players: sum(c, 'players'),
      playersPlayed: sum(c.filter(r => r.day <= today), 'players'),
      hours: sum(c, 'hours'),
      customers: people.size,
      returning: [...people.values()].filter(n => n > 1).length,
      online: c.filter(r => r.pay_method === 'online').length,
      walkIns: c.filter(r => r.pay_method === 'walkin').length,
      upcoming: c.filter(r => r.day >= today).length,
      months,
      byHour: [...Array(CLOSE - OPEN)].map((_, i) => c.filter(r => r.start_hour <= OPEN + i && r.start_hour + r.hours > OPEN + i).length),
      byWeekday: [...Array(7)].map((_, d) => c.filter(r => isoDow(r.day) === d).length),
    }
  }

  const routes = {
    'GET /api/availability': q => {
      const { today, hour } = now()
      const days = Math.min(62, Math.max(1, +(q.get('days') || 7)))
      const from = q.get('from') || today
      const booked = {}
      for (const r of db.rows) {
        const i = daysBetween(from, r.day)
        if (r.status !== 'confirmed' || i < 0 || i >= days) continue
        for (let h = r.start_hour; h < r.start_hour + r.hours; h++) (booked[r.day] ||= []).push(h)
      }
      Object.values(booked).forEach(h => h.sort((a, b) => a - b))
      return json(200, { open: OPEN, close: CLOSE, maxHours: MAX_BLOCK, today, hour, online: false, pricePerHour: null, booked })
    },
    'GET /api/account/me': () => {
      const u = me()
      if (!u) return fail(401, '')
      return ok({ user: { email: u.email, name: u.name, phone: u.phone, facebook: u.facebook, banned: !!u.banned_at, banReason: u.ban_reason } })
    },
    'POST /api/account/login': (_, b) => {
      const u = db.users.find(x => x.email.toLowerCase() === String(b.email || '').trim().toLowerCase())
      if (!u) return fail(401, 'Wrong email or password. In the demo, use the button above.')
      db.session.user = u.id
      return ok()
    },
    'POST /api/account/register': (_, b) => {
      const email = String(b.email || '').trim()
      if (db.users.some(x => x.email.toLowerCase() === email.toLowerCase())) return fail(400, 'That email already has an account. Sign in instead.')
      const u = { id: db.nextUser++, email, name: String(b.name || '').trim(), phone: String(b.phone || '').trim(), facebook: String(b.facebook || '').trim(), banned_at: null, ban_reason: '' }
      db.users.push(u)
      db.session.user = u.id
      return ok({ demo: true })
    },
    'POST /api/account/logout': () => { db.session.user = null; return ok() },
    'POST /api/account/forgot': () => ok(),
    'POST /api/account/resend': () => ok(),
    'POST /api/account/reset': () => ok(),
    'POST /api/account/profile': (_, b) => {
      const u = me()
      if (!u) return fail(401, 'Sign in again.')
      if (String(b.name || '').trim().length < 2) return fail(400, 'Enter your name.')
      Object.assign(u, { name: b.name.trim(), phone: String(b.phone || '').trim(), facebook: String(b.facebook || '').trim() })
      return ok()
    },
    'GET /api/account/reservations': () => {
      const u = me()
      if (!u) return fail(401, '')
      return ok({ items: db.rows.filter(r => r.user_id === u.id).sort((a, b) => sortRows(b, a)).map(publicRow) })
    },
    'POST /api/reservations': (_, b) => {
      const u = me()
      if (!u) return fail(401, 'Sign in to book.', { signIn: true })
      if (u.banned_at) return fail(403, 'Your account is blocked from booking. Message us if you think this is a mistake.', { banned: true })
      const slots = Array.isArray(b.slots) ? b.slots : []
      if (!slots.length) return fail(400, 'Pick an hour.')
      if (slots.length > MAX_PICKS) return fail(400, "That's too many hours for one request.")
      const { today, hour } = now()
      const want = blocks(slots)
      for (const w of want) {
        const ahead = daysBetween(today, w.day)
        if (ahead < 0 || ahead > 60 || (ahead === 0 && w.start_hour <= hour) || w.start_hour < OPEN || w.start_hour + w.hours > CLOSE) return fail(400, "That hour can't be booked anymore. Pick another.")
        if (db.rows.some(o => o.status === 'confirmed' && overlaps(o, w))) return fail(409, 'Someone already booked some of those hours. Pick another time.')
      }
      const ids = []
      for (const w of want) {
        const row = { id: db.nextId++, created_at: stamp(), name: u.name, phone: u.phone, facebook: u.facebook, ...w, players: +b.players || 4, note: String(b.note || ''), status: 'requested', pay_method: 'arrange', pay_by: null, paid_at: null, amount: null, decided_at: null, user_id: u.id, taken_at: null, bundle: null }
        db.rows.push(row)
        ids.push(row.id)
      }
      db.rows.filter(r => ids.includes(r.id)).forEach(r => { r.bundle = ids[0] })
      return ok({ id: ids[0], ids })
    },
    'GET /api/admin/me': () => {
      if (!db.session.admin) return fail(401, '')
      const { today, hour } = now()
      return ok({ today, hour, open: OPEN, close: CLOSE, db: true, online: false })
    },
    'POST /api/admin/login': () => { db.session.admin = true; return ok() },
    'POST /api/admin/logout': () => { db.session.admin = false; return ok() },
    'GET /api/admin/reservations': q => {
      if (!db.session.admin) return fail(401, '')
      const { today } = now()
      const scope = q.get('scope') || 'upcoming'
      const s = (q.get('q') || '').trim().toLowerCase()
      let rows = db.rows.filter(r => scope === 'all' || (scope === 'upcoming' ? r.day >= today : r.day < today))
      if (s) rows = rows.filter(r => [r.name, r.phone, r.facebook, r.note, r.day].some(v => v.toLowerCase().includes(s)) || `#${r.id}` === s || String(r.id) === s)
      if (scope === 'past') rows = rows.sort((a, b) => sortRows(b, a)).slice(0, 500)
      return ok({ items: rows.sort(sortRows).slice(0, 500).map(adminRow) })
    },
    'POST /api/admin/reservations': (_, b) => {
      if (!db.session.admin) return fail(401, '')
      const row = { id: db.nextId++, created_at: stamp(), name: String(b.name || '').trim(), phone: String(b.phone || '').trim(), facebook: String(b.facebook || '').trim(), day: b.date, start_hour: +b.hour, hours: +b.hours || 1, players: +b.players || 4, note: String(b.note || ''), status: 'requested', pay_method: 'walkin', pay_by: null, paid_at: null, amount: null, decided_at: null, user_id: null, taken_at: null, bundle: null }
      if (row.name.length < 2) return fail(400, 'Enter your name.')
      if (!row.phone && !row.facebook) return fail(400, 'Leave a phone number or a Facebook name so we can reach you.')
      if (row.start_hour + row.hours > CLOSE) return fail(400, 'Pick hours inside opening time.')
      row.bundle = row.id
      db.rows.push(row)
      const taken = confirm(row, !!b.paid)
      if (taken === null) {
        db.rows.pop()
        return fail(409, 'Those hours are already confirmed for someone else.')
      }
      return ok({ id: row.id, taken })
    },
    'POST /api/admin/reservations/:id': (_, b, id) => {
      if (!db.session.admin) return fail(401, '')
      const row = db.rows.find(r => r.id === +id)
      if (!row) return fail(404, 'Not found.')
      if (typeof b.paid === 'boolean') row.paid_at = b.paid ? row.paid_at || stamp() : null
      if (!b.status) return ok()
      if (b.status === 'confirmed') {
        const taken = confirm(row, !!b.paid)
        return taken === null ? fail(409, 'Those hours are already confirmed for someone else. Decline one first.') : ok({ taken })
      }
      row.status = b.status
      row.decided_at = stamp()
      if (b.status === 'awaiting' && b.payByHours) row.pay_by = new Date(Date.now() + b.payByHours * 3600e3).toISOString()
      reopen(row.day)
      return ok()
    },
    'POST /api/admin/users/:id/ban': (_, b, id) => {
      if (!db.session.admin) return fail(401, '')
      const u = db.users.find(x => x.id === +id)
      if (!u) return fail(404, 'Not found.')
      u.banned_at = b.ban === false ? null : u.banned_at || stamp()
      u.ban_reason = b.ban === false ? '' : String(b.reason || '').slice(0, 200)
      return ok()
    },
    'GET /api/admin/calendar': q => {
      if (!db.session.admin) return fail(401, '')
      const from = q.get('from'), to = q.get('to')
      return ok({ items: db.rows.filter(r => r.status === 'confirmed' && r.day >= from && r.day <= to).sort(sortRows).map(adminRow) })
    },
    'GET /api/admin/stats': () => db.session.admin ? ok({ open: OPEN, stats: stats() }) : fail(401, ''),
  }

  function route(method, path) {
    for (const [key, fn] of Object.entries(routes)) {
      const [m, pattern] = key.split(' ')
      if (m !== method) continue
      const re = new RegExp('^' + pattern.replace(/:id/g, '(\\d+)') + '$')
      const hit = path.match(re)
      if (hit) return [fn, hit[1]]
    }
    return [null]
  }

  window.fetch = async (input, init = {}) => {
    const url = new URL(typeof input === 'string' ? input : input.url, location.href)
    let path = url.pathname
    if (BASE && path.startsWith(BASE)) path = path.slice(BASE.length)
    if (url.origin !== location.origin || !path.startsWith('/api/')) return realFetch(input, init)
    const method = (init.method || 'GET').toUpperCase()
    const [fn, id] = route(method, path)
    if (!fn) return fail(404, 'The demo does not have this one.')
    let body = {}
    try { body = init.body ? JSON.parse(init.body) : {} } catch {}
    const res = fn(url.searchParams, body, id)
    save()
    showCount()
    return res
  }

  function skipButton(where, label, onClick) {
    const box = document.getElementById(where)
    if (!box) return
    const wrap = document.createElement('div')
    wrap.className = 'demo-skip'
    wrap.innerHTML = `<button type="button" class="btn bg-dink text-white hover:bg-dink-dark w-full"></button><p>Demo: skip sign-in and look around. Nothing here is real.</p>`
    wrap.querySelector('button').textContent = label
    wrap.querySelector('button').onclick = onClick
    box.prepend(wrap)
  }

  function inbox() {
    const d = document.createElement('dialog')
    d.className = 'ask'
    d.innerHTML = `<p class="ask-text">Emails the demo sent</p><ul class="mt-4 grid max-h-80 gap-3 overflow-y-auto text-sm"></ul><div class="ask-actions"><button type="button" class="btn-primary">Close</button></div>`
    const list = d.querySelector('ul')
    if (!db.outbox.length) list.innerHTML = '<li class="text-muted">None yet. Confirm a request in the admin that overlaps someone else\'s, and they get one.</li>'
    for (const m of db.outbox) {
      const li = document.createElement('li')
      li.className = 'rounded-xl border border-line p-3'
      li.innerHTML = '<p class="text-xs text-muted"></p><p class="font-semibold text-ink"></p><p class="mt-1"></p>'
      const [to, subject, text] = li.querySelectorAll('p')
      to.textContent = `To ${m.to}`
      subject.textContent = m.subject
      text.textContent = m.text
      list.append(li)
    }
    const close = () => { d.close(); d.remove() }
    d.querySelector('button').onclick = close
    d.addEventListener('click', e => { if (e.target === d) close() })
    document.body.append(d)
    d.showModal()
  }

  function badge() {
    const el = document.createElement('div')
    el.className = 'demo-badge'
    el.innerHTML = `<b>Demo</b><a href="${BASE}/book">Book</a><a href="${BASE}/account">Account</a><a href="${BASE}/admin">Admin</a><button type="button" data-inbox>Emails</button><button type="button" data-reset>Reset</button>`
    el.querySelector('[data-inbox]').onclick = inbox
    el.querySelector('[data-reset]').onclick = () => { localStorage.removeItem(KEY); location.reload() }
    document.body.append(el)
    showCount()
  }

  function showCount() {
    const b = document.querySelector('.demo-badge [data-inbox]')
    if (b) b.textContent = db.outbox.length ? `Emails (${db.outbox.length})` : 'Emails'
  }

  document.addEventListener('click', e => {
    const a = e.target.closest('#cal-download')
    if (!a) return
    e.preventDefault()
    window.ask?.('The calendar picture downloads on the real site. The demo has no server to draw it.', { ok: 'OK', cancel: 'Close' })
  }, true)

  document.addEventListener('DOMContentLoaded', () => {
    badge()
    skipButton('view-auth', 'Continue as Demo Player', () => {
      db.session.user = 1
      save()
      location.reload()
    })
    skipButton('login', 'Open the demo admin', () => {
      db.session.admin = true
      save()
      location.reload()
    })
  })
})()
