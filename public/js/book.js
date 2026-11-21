(() => {
  const floor = document.getElementById('floor')
  if (!floor) return

  const OPEN = 6
  const CLOSE = 23
  const DAYS = 7
  const HOUR = 3600e3
  const SENT_KEY = 'skibidink.requested'
  const POS = [{ left: 14, top: 20, width: 72, height: 47.6 }]
  const COURTS = POS.length
  const courtName = c => COURTS === 1 ? 'The court' : `Court ${c + 1}`
  const PLAYERS = [[16, 28], [24, 64], [72, 30], [80, 62]]

  const $ = id => document.getElementById(id)
  const MAX_PICKS = 8
  const state = { court: 0, day: 0, picks: new Set() }
  const sent = new Set(JSON.parse(localStorage.getItem(SENT_KEY) || '[]'))
  let live = null
  let user = null
  const toAccount = () => { location.href = sitePath('/account?next=' + encodeURIComponent('/book')) }

  const manila = new Intl.DateTimeFormat('en-CA', {
    timeZone: 'Asia/Manila', year: 'numeric', month: '2-digit', day: '2-digit',
    hour: '2-digit', minute: '2-digit', hourCycle: 'h23',
  })

  function now() {
    const p = Object.fromEntries(manila.formatToParts(new Date()).map(x => [x.type, x.value]))
    return { date: `${p.year}-${p.month}-${p.day}`, hour: +p.hour, minute: +p.minute, ms: Date.now() }
  }

  function addDays(date, n) {
    const d = new Date(`${date}T00:00:00+08:00`)
    d.setUTCDate(d.getUTCDate() + n)
    return new Intl.DateTimeFormat('en-CA', { timeZone: 'Asia/Manila' }).format(d)
  }

  const slotStart = (date, hour) => Date.parse(`${date}T${String(hour).padStart(2, '0')}:00:00+08:00`)

  function rand(key) {
    let h = 2166136261
    for (let i = 0; i < key.length; i++) h = Math.imul(h ^ key.charCodeAt(i), 16777619)
    h += 0x6d2b79f5
    h = Math.imul(h ^ (h >>> 15), h | 1)
    h ^= h + Math.imul(h ^ (h >>> 7), h | 61)
    return ((h ^ (h >>> 14)) >>> 0) / 4294967296
  }

  function demand(date, court, hour) {
    const dow = new Date(`${date}T12:00:00+08:00`).getUTCDay()
    let p = 0.38
    if (hour >= 17 && hour <= 20) p += 0.42
    else if (hour <= 8) p += 0.3
    else if (hour >= 11 && hour <= 14) p -= 0.08
    if (dow === 0 || dow === 6) p += 0.15
    return p
  }

  /** When this slot gets booked, or null when it stays open. The same for every visitor. */
  function bookedAt(date, court, hour) {
    const key = `${date}|${court}|${hour}`
    if (rand(key) >= demand(date, court, hour)) return null
    const lead = Math.pow(rand(key + '|t'), 1.3) * 48
    return slotStart(date, hour) - lead * HOUR
  }

  const isBooked = (t, date, court, hour) => {
    if (live) return (live.booked[date] || []).includes(hour)
    const at = bookedAt(date, court, hour)
    return at !== null && at <= t.ms
  }

  async function loadLive() {
    try {
      const res = await fetch('/api/availability?days=' + DAYS, { cache: 'no-store' })
      if (!res.ok) return false
      live = await res.json()
      return true
    } catch {
      return false
    }
  }

  const fmtHour = h => `${h % 12 || 12} ${h < 12 ? 'AM' : 'PM'}`
  const dayName = (date, i) => i === 0 ? 'Today'
    : new Date(`${date}T12:00:00+08:00`).toLocaleDateString('en-PH', { weekday: 'short', timeZone: 'Asia/Manila' })
  const dayNum = date => +date.slice(8)
  const longDate = date => new Date(`${date}T12:00:00+08:00`).toLocaleDateString('en-PH', { weekday: 'long', month: 'short', day: 'numeric', timeZone: 'Asia/Manila' })

  function playUntil(t, court) {
    let h = t.hour
    while (h < CLOSE && isBooked(t, t.date, court, h)) h++
    return h
  }

  function buildCourts() {
    POS.forEach((pos, i) => {
      const b = document.createElement('button')
      b.type = 'button'
      b.className = 'court'
      b.style.left = pos.left + '%'
      b.style.top = pos.top + '%'
      b.style.width = pos.width + '%'
      b.style.height = pos.height + '%'
      b.dataset.court = i
      b.innerHTML = `
        <div class="court-play">
          <div class="court-kitchen"></div>
          <div class="court-line left-0 top-1/2 h-0.5 w-[34.1%] -translate-y-1/2"></div>
          <div class="court-line right-0 top-1/2 h-0.5 w-[34.1%] -translate-y-1/2"></div>
          <div class="court-line inset-y-0 left-[34.1%] w-0.5"></div>
          <div class="court-line inset-y-0 right-[34.1%] w-0.5"></div>
          <div class="court-net"></div>
          ${PLAYERS.map(([x, y], n) => `<span class="player" style="left:${x}%;top:${y}%;animation-delay:${-n * 0.7}s"></span>`).join('')}
          <span class="ball" style="animation-delay:${-i * 0.45}s"></span>
        </div>
        <span class="court-tag"><i></i><span></span></span>
        ${COURTS > 1 ? `<span class="court-name">${courtName(i)}</span>` : ""}`
      b.addEventListener('click', () => {
        state.court = i
        state.picks.clear()
        render()
      })
      floor.appendChild(b)
    })
  }

  function renderFloor(t) {
    const openHours = t.hour >= OPEN && t.hour < CLOSE
    let play = 0
    floor.querySelectorAll('.court').forEach(b => {
      const c = +b.dataset.court
      const busy = openHours && isBooked(t, t.date, c, t.hour)
      if (busy) play++
      b.classList.toggle('is-play', busy)
      b.classList.toggle('is-picked', COURTS > 1 && c === state.court)
      b.querySelector('.court-tag span').textContent = !openHours ? 'Closed' : busy ? `In play · till ${fmtHour(playUntil(t, c))}` : 'Open now'
      b.setAttribute('aria-label', `${courtName(c)}, ${!openHours ? 'closed' : busy ? 'in play' : 'open now'}`)
      b.setAttribute('aria-pressed', c === state.court)
    })
    let next = null
    for (let h = Math.max(OPEN, t.hour + 1); h < CLOSE && next === null; h++) {
      for (let c = 0; c < COURTS; c++) if (!isBooked(t, t.date, c, h)) { next = h; break }
    }
    $('stat-play').textContent = !openHours ? 'Closed' : play === COURTS ? 'In play' : COURTS === 1 ? 'Open' : `${COURTS - play} open`
    $('stat-open').textContent = next === null ? '—' : fmtHour(next)
    $('clock').textContent = `${t.hour % 12 || 12}:${String(t.minute).padStart(2, '0')} ${t.hour < 12 ? 'AM' : 'PM'}`
  }

  const pickKey = (date, hour) => `${date}|${hour}`
  const sentKey = (date, hour) => `${date}|${state.court}|${hour}`

  function freeHours(t, i) {
    const date = addDays(t.date, i)
    const hours = []
    for (let h = OPEN; h < CLOSE; h++) if (!(i === 0 && h <= t.hour) && !isBooked(t, date, state.court, h)) hours.push(h)
    return hours
  }

  function blocks() {
    const out = []
    for (const k of [...state.picks].sort((a, b) => a.localeCompare(b, 'en', { numeric: true }))) {
      const [date, h] = [k.slice(0, 10), +k.slice(11)]
      const last = out[out.length - 1]
      if (last && last.date === date && last.hour + last.hours === h) last.hours++
      else out.push({ date, hour: h, hours: 1 })
    }
    return out
  }

  const byDate = bs => [...new Set(bs.map(b => b.date))].map(d => [d, bs.filter(b => b.date === d)])
  const blockText = b => `${fmtHour(b.hour)}–${fmtHour(b.hour + b.hours)}`

  function pruneDays(t, open) {
    for (const k of state.picks) {
      const i = Math.round((Date.parse(`${k.slice(0, 10)}T00:00:00+08:00`) - Date.parse(`${t.date}T00:00:00+08:00`)) / 864e5)
      if (i < 0 || i >= DAYS || !open[i].includes(+k.slice(11))) state.picks.delete(k)
    }
    if (!open[state.day].length) {
      const next = open.findIndex(h => h.length)
      if (next >= 0) state.day = next
    }
  }

  function renderDays(t) {
    const open = [...Array(DAYS)].map((_, i) => freeHours(t, i))
    pruneDays(t, open)
    const box = $('days')
    box.innerHTML = ''
    for (let i = 0; i < DAYS; i++) {
      const date = addDays(t.date, i)
      const full = !open[i].length
      const b = document.createElement('button')
      b.type = 'button'
      b.className = 'day'
      b.disabled = full
      b.setAttribute('role', 'tab')
      b.setAttribute('aria-selected', i === state.day && !full)
      b.title = full ? (i === 0 && t.hour >= CLOSE - 1 ? 'No more hours today' : 'Fully booked') : ''
      b.innerHTML = `<span class="text-[11px] font-semibold uppercase">${dayName(date, i)}</span><span class="font-display text-lg font-bold">${dayNum(date)}</span>${full ? '<span class="day-stamp">Full</span>' : ''}`
      b.addEventListener('click', () => {
        state.day = i
        render()
      })
      box.appendChild(b)
    }
    return open
  }

  function updateDaysHint() {
    const box = $('days')
    box.parentElement.classList.toggle('can-scroll', box.scrollLeft + box.clientWidth < box.scrollWidth - 4)
  }
  $('days').addEventListener('scroll', updateDaysHint, { passive: true })
  addEventListener('resize', updateDaysHint)

  function renderSlots(t, open) {
    const date = addDays(t.date, state.day)
    const box = $('slots')
    box.innerHTML = ''
    for (let h = OPEN; h < CLOSE; h++) {
      if (state.day === 0 && h <= t.hour) continue
      const b = document.createElement('button')
      b.type = 'button'
      b.className = 'slot'
      b.textContent = fmtHour(h)
      const key = pickKey(date, h)
      const held = sent.has(sentKey(date, h))
      const booked = isBooked(t, date, state.court, h)
      if (booked) b.classList.add('is-booked')
      if (held && !booked) b.classList.add('is-held')
      b.disabled = booked
      b.setAttribute('aria-pressed', state.picks.has(key))
      b.title = booked ? 'Booked' : held ? 'You sent a request for this hour' : 'Open'
      b.addEventListener('click', () => {
        if (state.picks.has(key)) state.picks.delete(key)
        else if (state.picks.size >= MAX_PICKS) return toast(`Up to ${MAX_PICKS} hours in one request.`)
        else state.picks.add(key)
        render()
      })
      box.appendChild(b)
    }
    if (!box.children.length) box.innerHTML = `<p class="col-span-4 grid min-h-[12.625rem] place-items-center rounded-lg bg-slate-100 text-center text-sm text-muted">No more hours this week. Check back soon.</p>`

    $('pick-court').textContent = courtName(state.court)
    const status = $('pick-status')
    const free = open[state.day].length
    status.textContent = free ? `${free} open` : 'Full'
    status.className = `rounded-full px-2.5 py-1 text-xs font-bold ${free ? 'bg-skibi/10 text-skibi' : 'bg-slate-100 text-muted'}`

    const summary = $('summary')
    const hold = $('hold')
    const n = state.picks.size
    if (!n) {
      summary.textContent = !live ? 'Pick open hours to hold them.' : user ? 'Pick one or more open hours to request them.' : 'Pick open hours, then sign in to request them.'
      summary.className = 'rounded-xl border border-dashed border-slate-300 p-4 text-sm text-muted'
      hold.disabled = true
      hold.textContent = live ? 'Request these hours' : 'Hold these hours'
      return
    }
    const picked = blocks()
    const byDay = byDate(picked)
    summary.innerHTML = `<span class="block text-xs uppercase tracking-wider text-muted">Your pick · ${n} hour${n > 1 ? 's' : ''}</span>` +
      byDay.map(([d, bs]) => `<span class="mt-1 block"><b class="font-display text-ink">${bs.map(blockText).join(', ')}</b> <span class="text-muted">${longDate(d)}</span></span>`).join('')
    summary.className = 'rounded-xl border border-skibi/40 bg-skibi/5 p-4 text-sm'
    hold.disabled = false
    const label = `${n} hour${n > 1 ? 's' : ''}`
    if (live && user?.banned) {
      hold.disabled = true
      hold.textContent = 'Your account is blocked from booking'
      return
    }
    if (live) {
      hold.textContent = !user ? 'Sign in to book' : `Request ${label}`
      hold.onclick = () => user ? openForm() : toAccount()
      return
    }
    const keys = [...state.picks].map(k => sentKey(k.slice(0, 10), +k.slice(11)))
    const allHeld = keys.every(k => sent.has(k))
    hold.textContent = allHeld ? `Release ${label}` : `Hold ${label}`
    hold.onclick = () => {
      keys.forEach(k => allHeld ? sent.delete(k) : sent.add(k))
      localStorage.setItem(SENT_KEY, JSON.stringify([...sent]))
      toast(allHeld ? 'Hold released.' : `Held ${label}. Confirmation opens soon.`)
      state.picks.clear()
      render()
    }
  }

  const form = $('book-form')
  const dialog = $('book-dialog')

  function openForm() {
    const picked = blocks()
    const n = state.picks.size
    $('book-when').innerHTML = byDate(picked)
      .map(([d, bs]) => `<span class="block">${longDate(d)}: ${bs.map(blockText).join(', ')}</span>`).join('')
    $('book-who').textContent = [user.name, user.phone].filter(Boolean).join(' · ')
    $('pay-online').classList.toggle('hidden', !live.online)
    $('pay-price').textContent = live.pricePerHour ? `₱${(live.pricePerHour * n).toLocaleString('en-PH')} for ${n} hour${n > 1 ? 's' : ''}. ` : ''
    form.querySelector('[name=pay][value=arrange]').checked = true
    $('book-error').textContent = ''
    $('book-done').classList.add('hidden')
    form.classList.remove('hidden')
    dialog.showModal()
  }

  if (form) {
    $('book-cancel').addEventListener('click', () => dialog.close())
    $('book-close').addEventListener('click', () => dialog.close())
    form.addEventListener('submit', async e => {
      e.preventDefault()
      const slots = [...state.picks].map(k => ({ date: k.slice(0, 10), hour: +k.slice(11) }))
      const body = { ...Object.fromEntries(new FormData(form)), slots }
      const button = form.querySelector('[type=submit]')
      button.disabled = true
      $('book-error').textContent = ''
      try {
        const res = await fetch('/api/reservations', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(body) })
        const data = await res.json().catch(() => ({}))
        if (data.signIn) return toAccount()
        if (data.banned) user.banned = true
        if (!res.ok) throw new Error(data.error || 'Could not send. Try again.')
        slots.forEach(p => sent.add(sentKey(p.date, p.hour)))
        state.picks.clear()
        localStorage.setItem(SENT_KEY, JSON.stringify([...sent]))
        if (data.checkoutUrl) {
          location.href = data.checkoutUrl
          return
        }
        form.classList.add('hidden')
        $('book-done-text').textContent = data.payFailed
          ? `Request #${data.id} is in, but online payment is down right now. We'll message you to arrange payment.`
          : `Request #${data.id} is in. We'll message you to arrange payment. The slot is yours once it's paid.`
        $('book-done').classList.remove('hidden')
        render()
      } catch (err) {
        $('book-error').textContent = err.message
        await loadLive()
        render()
      } finally {
        button.disabled = false
      }
    })
  }

  function returnFromCheckout() {
    const q = new URLSearchParams(location.search)
    if (q.has('paid')) toast(`Payment received for request #${q.get('paid')}. Your booking is confirmed once it clears.`)
    else if (q.has('unpaid')) toast(`Payment for request #${q.get('unpaid')} wasn't finished. We'll keep it as a request.`)
    else return
    history.replaceState(null, '', location.pathname)
  }

  let toastTimer
  function toast(text) {
    const el = $('toast')
    el.textContent = text
    el.classList.add('show')
    clearTimeout(toastTimer)
    toastTimer = setTimeout(() => el.classList.remove('show'), 3200)
  }

  function render() {
    const t = now()
    renderFloor(t)
    renderSlots(t, renderDays(t))
    updateDaysHint()
  }

  let lastKey = ''
  let ticks = 0
  async function tick() {
    if (live && ++ticks % 6 === 0) await loadLive()
    const t = now()
    const key = `${t.date}|${t.hour}|${JSON.stringify(live?.booked ?? '')}`
    if (key !== lastKey) render()
    else renderFloor(t)
    lastKey = key
  }

  buildCourts()
  render()
  returnFromCheckout()
  loadLive().then(async ok => {
    if (!ok) return
    user = await fetch('/api/account/me', { cache: 'no-store' }).then(r => r.ok ? r.json() : null).then(d => d?.user ?? null).catch(() => null)
    $('board-note').textContent = live.online ? 'Pay online and the slot is yours at once, or send a request and pay us directly.' : 'Send a request and we\'ll message you to arrange payment. The slot is yours once paid.'
    render()
  })
  setInterval(tick, 10e3)
})()
