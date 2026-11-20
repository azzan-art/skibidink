(() => {
  const floor = document.getElementById('floor')
  if (!floor) return

  const COURTS = 4
  const OPEN = 6
  const CLOSE = 22
  const DAYS = 7
  const HOUR = 3600e3
  const HELD_KEY = 'skibidink.held'
  const POS = [
    { left: 3, top: 19 }, { left: 51, top: 19 },
    { left: 3, top: 53 }, { left: 51, top: 53 },
  ]
  const PLAYERS = [[16, 28], [24, 64], [72, 30], [80, 62]]

  const $ = id => document.getElementById(id)
  const state = { court: 0, day: 0, hour: null, seen: new Set() }
  let held = new Set(JSON.parse(localStorage.getItem(HELD_KEY) || '[]'))

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
    if (court === 0) p += 0.06
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
    const at = bookedAt(date, court, hour)
    return at !== null && at <= t.ms
  }

  const fmtHour = h => `${h % 12 || 12} ${h < 12 ? 'AM' : 'PM'}`
  const dayName = (date, i) => i === 0 ? 'Today' : i === 1 ? 'Tomorrow'
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
      b.style.width = '46%'
      b.style.height = '30%'
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
        <span class="court-name">Court ${i + 1}</span>`
      b.addEventListener('click', () => {
        state.court = i
        state.hour = null
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
      b.classList.toggle('is-picked', c === state.court)
      b.querySelector('.court-tag span').textContent = !openHours ? 'Closed' : busy ? `In play · till ${fmtHour(playUntil(t, c))}` : 'Open now'
      b.setAttribute('aria-label', `Court ${c + 1}, ${!openHours ? 'closed' : busy ? 'in play' : 'open now'}`)
      b.setAttribute('aria-pressed', c === state.court)
    })
    let left = 0
    for (let c = 0; c < COURTS; c++) {
      for (let h = Math.max(OPEN, t.hour + 1); h < CLOSE; h++) if (!isBooked(t, t.date, c, h)) left++
    }
    $('stat-play').textContent = play
    $('stat-open').textContent = openHours ? COURTS - play : 0
    $('stat-left').textContent = left
    $('clock').textContent = `Manila ${String(t.hour).padStart(2, '0')}:${String(t.minute).padStart(2, '0')}`
  }

  function renderDays(t) {
    const box = $('days')
    box.innerHTML = ''
    for (let i = 0; i < DAYS; i++) {
      const date = addDays(t.date, i)
      const b = document.createElement('button')
      b.type = 'button'
      b.className = 'day'
      b.setAttribute('role', 'tab')
      b.setAttribute('aria-selected', i === state.day)
      b.innerHTML = `<span class="text-[11px] font-semibold uppercase">${dayName(date, i)}</span><span class="font-display text-lg font-bold">${dayNum(date)}</span>`
      b.addEventListener('click', () => {
        state.day = i
        state.hour = null
        render()
      })
      box.appendChild(b)
    }
  }

  function renderSlots(t) {
    const date = addDays(t.date, state.day)
    const box = $('slots')
    box.innerHTML = ''
    for (let h = OPEN; h < CLOSE; h++) {
      const b = document.createElement('button')
      b.type = 'button'
      b.className = 'slot'
      b.textContent = fmtHour(h)
      const key = `${date}|${state.court}|${h}`
      const past = state.day === 0 && h <= t.hour
      const booked = isBooked(t, date, state.court, h)
      if (booked) b.classList.add('is-booked')
      if (state.seen.has(key)) b.classList.add('just-booked')
      if (held.has(key) && !booked) b.classList.add('is-held')
      b.disabled = past || booked
      b.setAttribute('aria-pressed', state.hour === h)
      b.title = past ? 'Past' : booked ? 'Booked' : held.has(key) ? 'Held by you' : 'Open'
      b.addEventListener('click', () => {
        state.hour = h
        render()
      })
      box.appendChild(b)
    }
    state.seen.clear()

    $('pick-court').textContent = `Court ${state.court + 1}`
    const status = $('pick-status')
    const free = [...Array(CLOSE - OPEN)].filter((_, k) => !(state.day === 0 && OPEN + k <= t.hour) && !isBooked(t, date, state.court, OPEN + k)).length
    status.textContent = `${free} open`
    status.className = `rounded-full px-2.5 py-1 text-xs font-bold ${free ? 'bg-ball/15 text-ball' : 'bg-white/10 text-slate-400'}`

    const summary = $('summary')
    const hold = $('hold')
    if (state.hour === null) {
      summary.textContent = 'Pick an open hour to hold it.'
      summary.className = 'rounded-xl border border-dashed border-white/15 p-4 text-sm text-slate-400'
      hold.disabled = true
      hold.textContent = 'Hold this slot'
      return
    }
    const key = `${date}|${state.court}|${state.hour}`
    summary.innerHTML = `<span class="block text-xs uppercase tracking-wider text-slate-400">Your pick</span>
      <span class="mt-1 block font-display text-lg font-bold text-white">Court ${state.court + 1} · ${fmtHour(state.hour)}–${fmtHour(state.hour + 1)}</span>
      <span class="text-slate-400">${longDate(date)}</span>`
    summary.className = 'rounded-xl border border-ball/40 bg-ball/5 p-4 text-sm'
    hold.disabled = false
    hold.textContent = held.has(key) ? 'Release this hold' : 'Hold this slot'
    hold.onclick = () => {
      if (held.has(key)) held.delete(key)
      else held.add(key)
      localStorage.setItem(HELD_KEY, JSON.stringify([...held]))
      toast(held.has(key) ? `Held Court ${state.court + 1} at ${fmtHour(state.hour)}. Confirmation opens soon.` : 'Hold released.')
      render()
    }
  }

  function recentBookings(t) {
    const out = []
    for (let i = 0; i < 3; i++) {
      const date = addDays(t.date, i)
      for (let c = 0; c < COURTS; c++) {
        for (let h = OPEN; h < CLOSE; h++) {
          const at = bookedAt(date, c, h)
          if (at !== null && at <= t.ms && at > t.ms - 12 * HOUR && slotStart(date, h) > t.ms) out.push({ at, date, c, h, i })
        }
      }
    }
    return out.sort((a, b) => b.at - a.at).slice(0, 5)
  }

  function ago(ms) {
    const m = Math.round(ms / 60e3)
    return m < 1 ? 'just now' : m < 60 ? `${m} min ago` : `${Math.round(m / 60)} hr ago`
  }

  let lastTop = null
  function renderFeed(t) {
    const items = recentBookings(t)
    const top = items[0] && `${items[0].date}|${items[0].c}|${items[0].h}`
    if (lastTop !== null && top !== lastTop && items[0]) {
      state.seen.add(top)
      toast(`Court ${items[0].c + 1} at ${fmtHour(items[0].h)} just got booked`)
    }
    lastTop = top ?? ''
    $('feed').innerHTML = items.map(b => {
      const when = b.i === 0 ? (b.h >= 17 ? 'tonight' : 'today') : b.i === 1 ? 'tomorrow' : dayName(b.date, b.i)
      return `<li class="feed-item"><span class="size-1.5 rounded-full bg-amber-400"></span>Court ${b.c + 1} · ${fmtHour(b.h)} ${when}<span class="text-slate-500">· booked ${ago(t.ms - b.at)}</span></li>`
    }).join('')
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
    renderDays(t)
    renderSlots(t)
  }

  let lastKey = ''
  function tick() {
    const t = now()
    renderFeed(t)
    const key = `${t.date}|${t.hour}|${lastTop}`
    if (key !== lastKey) render()
    else renderFloor(t)
    lastKey = key
  }

  buildCourts()
  renderFeed(now())
  render()
  setInterval(tick, 10e3)
})()
