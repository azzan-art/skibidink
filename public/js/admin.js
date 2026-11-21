(() => {
  const $ = id => document.getElementById(id)
  const esc = s => String(s ?? '').replace(/[&<>"']/g, c => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' })[c])
  const fmtHour = h => `${h % 12 || 12} ${h < 12 || h === 24 ? 'AM' : 'PM'}`
  const span = r => `${fmtHour(r.start_hour)}–${fmtHour(r.start_hour + r.hours)}`
  const dateOf = iso => new Date(`${iso}T12:00:00Z`)
  const isoOf = d => d.toISOString().slice(0, 10)
  const addDays = (iso, n) => { const d = dateOf(iso); d.setUTCDate(d.getUTCDate() + n); return isoOf(d) }
  const longDate = iso => dateOf(iso).toLocaleDateString('en-PH', { weekday: 'long', month: 'long', day: 'numeric', year: 'numeric', timeZone: 'UTC' })
  const shortDate = iso => dateOf(iso).toLocaleDateString('en-PH', { weekday: 'short', month: 'short', day: 'numeric', timeZone: 'UTC' })
  const monthName = iso => dateOf(iso).toLocaleDateString('en-PH', { month: 'long', year: 'numeric', timeZone: 'UTC' })
  const mondayOf = iso => addDays(iso, -((dateOf(iso).getUTCDay() + 6) % 7))
  const when = ts => ts ? new Date(ts).toLocaleString('en-PH', { month: 'short', day: 'numeric', hour: 'numeric', minute: '2-digit', timeZone: 'Asia/Manila' }) : ''
  const STATUS_LABEL = { requested: 'Requested', awaiting: 'Awaiting payment', confirmed: 'Confirmed', declined: 'Declined', cancelled: 'Cancelled' }
  const PAY_LABEL = { online: 'Pays online', arrange: 'Pay arranged with us', walkin: 'Walk-in' }

  const state = { me: null, tab: 'requests', filter: 'open', items: [], cal: { view: 'month', date: null } }

  let toastTimer
  function toast(text) {
    const el = $('toast')
    el.textContent = text
    el.classList.add('show')
    clearTimeout(toastTimer)
    toastTimer = setTimeout(() => el.classList.remove('show'), 3200)
  }

  async function api(path, body) {
    const res = await fetch(path, body === undefined ? {} : { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(body) })
    const data = await res.json().catch(() => ({}))
    if (res.status === 401 && path !== '/api/admin/login') showLogin()
    if (!res.ok) throw new Error(data.error || 'Something went wrong.')
    return data
  }

  function showLogin() {
    state.me = null
    $('login').classList.remove('hidden')
    $('tabs').classList.add('hidden')
    $('logout').classList.add('hidden')
    document.querySelectorAll('.admin-panel').forEach(p => p.classList.add('hidden'))
  }

  async function start() {
    try {
      state.me = await api('/api/admin/me')
    } catch {
      return showLogin()
    }
    state.cal.date = state.me.today
    $('login').classList.add('hidden')
    $('tabs').classList.remove('hidden')
    $('tabs').classList.add('flex')
    $('logout').classList.remove('hidden')
    if (!state.me.db) toast('No database connected: set DATABASE_URL.')
    const hourSelect = document.querySelector('[data-hours]')
    hourSelect.innerHTML = ''
    for (let h = state.me.open; h < state.me.close; h++) hourSelect.add(new Option(fmtHour(h), h))
    show(location.hash.slice(1) || 'requests')
  }

  function show(tab) {
    if (!['requests', 'calendar', 'dashboard'].includes(tab)) tab = 'requests'
    state.tab = tab
    history.replaceState(null, '', `#${tab}`)
    document.querySelectorAll('.admin-tab').forEach(b => b.setAttribute('aria-selected', b.dataset.tab === tab))
    document.querySelectorAll('.admin-panel').forEach(p => p.classList.toggle('hidden', p.id !== `panel-${tab}`))
    if (tab === 'requests') loadList()
    if (tab === 'calendar') loadCalendar()
    if (tab === 'dashboard') loadStats()
  }

  // Requests

  const FILTERS = [
    ['open', 'Needs action', r => r.status === 'requested' || r.status === 'awaiting'],
    ['requested', 'Requested', r => r.status === 'requested'],
    ['awaiting', 'Awaiting payment', r => r.status === 'awaiting'],
    ['confirmed', 'Confirmed', r => r.status === 'confirmed'],
    ['closed', 'Declined', r => r.status === 'declined' || r.status === 'cancelled'],
    ['all', 'All', () => true],
  ]

  let searchTimer
  async function loadList() {
    const q = encodeURIComponent($('search').value.trim())
    try {
      state.items = (await api(`/api/admin/reservations?scope=${$('scope').value}&q=${q}`)).items
    } catch (e) {
      $('list').innerHTML = `<p class="text-sm text-red-600">${esc(e.message)}</p>`
      return
    }
    renderList()
  }

  function contactLinks(r) {
    const out = []
    if (r.phone) out.push(`<a class="font-semibold text-dink hover:underline" href="tel:${esc(r.phone.replace(/[^\d+]/g, ''))}">${esc(r.phone)}</a>`)
    if (r.facebook) {
      const url = /^https?:\/\//i.test(r.facebook) ? r.facebook : /facebook\.com|fb\.com|m\.me/i.test(r.facebook) ? `https://${r.facebook}` : null
      out.push(url ? `<a class="font-semibold text-dink hover:underline" href="${esc(url)}" target="_blank" rel="noopener noreferrer">Facebook</a>` : `<span>FB: ${esc(r.facebook)}</span>`)
    }
    return out.join('<span class="text-line">|</span>')
  }

  function actions(r) {
    const b = (label, cls, act) => `<button type="button" class="act ${cls}" data-id="${r.id}" data-act="${act}">${label}</button>`
    const block = !r.user_id ? '' : `<button type="button" class="act ml-auto ${r.banned_at ? '' : 'act-block'}" data-user="${r.user_id}" data-act="${r.banned_at ? 'unban' : 'ban'}">${r.banned_at ? 'Unblock account' : 'Block account'}</button>`
    return statusActions(r, b) + block
  }

  function statusActions(r, b) {
    switch (r.status) {
      case 'requested':
        return b('Awaiting payment', '', 'awaiting') + b('Paid: confirm', 'act-go', 'confirm-paid') + b('Confirm, pay at court', '', 'confirm') + b('Decline', '', 'declined')
      case 'awaiting':
        return b('Paid: confirm', 'act-go', 'confirm-paid') + b('Confirm, pay at court', '', 'confirm') + b('Extend 24 h', '', 'extend') + b('Decline', '', 'declined')
      case 'confirmed':
        return (r.paid_at ? b('Mark unpaid', '', 'unpaid') : b('Mark paid', 'act-pay', 'paid')) + b('Cancel booking', '', 'cancelled')
      default:
        return b('Reopen', '', 'requested')
    }
  }

  function card(r) {
    const overdue = r.status === 'awaiting' && r.pay_by && new Date(r.pay_by) < new Date()
    const rivals = r.competing.filter(o => o.status !== 'confirmed')
    const taken = r.competing.find(o => o.status === 'confirmed')
    let warn = ''
    if (taken && r.status !== 'confirmed' && r.status !== 'declined' && r.status !== 'cancelled') {
      warn = `<p class="mt-3 rounded-lg bg-red-50 px-3 py-2 text-xs font-semibold text-red-700">These hours are already confirmed for ${esc(taken.name)}${r.paid_at ? '. This one paid online too: refund or move it.' : '. Decline or move this one.'}</p>`
    } else if (rivals.length && r.status !== 'declined' && r.status !== 'cancelled') {
      warn = `<div class="mt-3 rounded-lg bg-amber-50 px-3 py-2 text-xs text-amber-900">
        <p class="font-bold">${rivals.length === 1 ? '1 other request wants' : `${rivals.length} other requests want`} these hours. First to pay gets it:</p>
        <ul class="mt-1 grid gap-0.5">${rivals.map(o => `<li>#${o.id} ${esc(o.name)} · ${fmtHour(o.start_hour)}–${fmtHour(o.start_hour + o.hours)} · ${contactLinks(o) || 'no contact'} · ${STATUS_LABEL[o.status]}</li>`).join('')}</ul>
      </div>`
    }
    const meta = [
      `${r.players} player${r.players === 1 ? '' : 's'}`,
      PAY_LABEL[r.pay_method] || r.pay_method,
      r.amount ? `₱${(r.amount / 100).toLocaleString('en-PH')}` : '',
      r.paid_at ? `<span class="font-bold text-skibi-dark">Paid ${when(r.paid_at)}</span>` : '',
      r.status === 'awaiting' && r.pay_by ? `<span class="${overdue ? 'font-bold text-red-600' : ''}">Pay by ${when(r.pay_by)}${overdue ? ' (overdue)' : ''}</span>` : '',
      `Sent ${when(r.created_at)}`,
    ].filter(Boolean).join(' · ')
    return `<article class="rounded-2xl border ${overdue ? 'border-red-300' : 'border-line'} bg-white p-4 sm:p-5">
      <div class="flex flex-wrap items-start gap-x-4 gap-y-2">
        <div class="min-w-28"><p class="font-display text-xl font-bold text-ink">${span(r)}</p><p class="text-xs text-muted">#${r.id}</p></div>
        <div class="mr-auto min-w-0">
          <p class="font-bold text-ink">${esc(r.name)}${r.banned_at ? `<span class="ml-2 rounded bg-red-100 px-1.5 py-0.5 align-middle text-[11px] font-bold uppercase text-red-700" title="${esc(r.ban_reason || 'Blocked')}">Blocked</span>` : ''}</p>
          <p class="mt-0.5 flex flex-wrap gap-2 text-sm">${contactLinks(r)}${r.email ? `<span class="text-line">|</span><a class="text-muted hover:underline" href="mailto:${esc(r.email)}">${esc(r.email)}</a>` : ''}</p>
          <p class="mt-1 text-xs text-muted">${meta}</p>
          ${r.note ? `<p class="mt-2 text-sm text-body">“${esc(r.note)}”</p>` : ''}
        </div>
        <span class="status status-${r.status}">${STATUS_LABEL[r.status]}</span>
      </div>
      ${warn}
      <div class="mt-4 flex flex-wrap gap-2">${actions(r)}</div>
    </article>`
  }

  function renderList() {
    const counts = Object.fromEntries(FILTERS.map(([k, , f]) => [k, state.items.filter(f).length]))
    $('filters').innerHTML = FILTERS.map(([k, label]) => `<button type="button" class="chip" data-filter="${k}" aria-pressed="${state.filter === k}">${label} <span class="opacity-60">${counts[k]}</span></button>`).join('')
    const badge = $('tab-count')
    badge.textContent = counts.open
    badge.classList.toggle('hidden', !counts.open)
    const pick = FILTERS.find(([k]) => k === state.filter)[2]
    const rows = state.items.filter(pick)
    if (!rows.length) {
      $('list').innerHTML = `<p class="rounded-2xl border border-dashed border-slate-300 p-10 text-center text-sm text-muted">Nothing here${$('search').value ? ' for that search' : ''}.</p>`
      return
    }
    const days = []
    for (const r of rows) {
      if (!days.length || days[days.length - 1].day !== r.day) days.push({ day: r.day, rows: [] })
      days[days.length - 1].rows.push(r)
    }
    $('list').innerHTML = days.map(d => `<section>
      <h2 class="mb-3 text-sm font-bold uppercase tracking-wider text-muted">${d.day === state.me.today ? 'Today · ' : d.day === addDays(state.me.today, 1) ? 'Tomorrow · ' : ''}${longDate(d.day)}</h2>
      <div class="grid gap-3">${d.rows.map(card).join('')}</div>
    </section>`).join('')
  }

  async function act(id, what) {
    const body = {
      awaiting: { status: 'awaiting', payByHours: 24 },
      extend: { status: 'awaiting', payByHours: 24 },
      'confirm-paid': { status: 'confirmed', paid: true },
      confirm: { status: 'confirmed' },
      declined: { status: 'declined' },
      cancelled: { status: 'cancelled' },
      requested: { status: 'requested' },
      paid: { paid: true },
      unpaid: { paid: false },
    }[what]
    if (what === 'cancelled' && !await ask('Cancel this confirmed booking? The hours open up again.', { ok: 'Cancel booking', danger: true })) return
    try {
      const res = await api(`/api/admin/reservations/${id}`, body)
      const taken = res.taken ? ` Emailed ${res.taken === 1 ? 'the 1 player' : `the ${res.taken} players`} whose request it took.` : ''
      toast({ awaiting: 'Waiting for payment, 24 hours to pay.', extend: 'Pay-by moved 24 hours later.', 'confirm-paid': 'Confirmed and paid.', confirm: 'Confirmed.', declined: 'Declined.', cancelled: 'Booking cancelled.', requested: 'Reopened.', paid: 'Marked paid.', unpaid: 'Marked unpaid.' }[what] + taken)
    } catch (e) {
      toast(e.message)
    }
    loadList()
  }

  // Calendar

  function calRange() {
    const { view, date } = state.cal
    if (view === 'day') return [date, date]
    if (view === 'week') { const s = mondayOf(date); return [s, addDays(s, 6)] }
    const first = date.slice(0, 8) + '01'
    const next = addDays(first, 32).slice(0, 8) + '01'
    return [mondayOf(first), addDays(mondayOf(addDays(next, -1)), 6)]
  }

  async function loadCalendar() {
    const { view, date } = state.cal
    document.querySelectorAll('.cal-view').forEach(b => b.setAttribute('aria-selected', b.dataset.view === view))
    const [from, to] = calRange()
    $('cal-title').textContent = view === 'day' ? longDate(date) : view === 'week' ? `${shortDate(from)} – ${shortDate(to)}` : monthName(date)
    const poster = view === 'day' ? `day?date=${date}` : view === 'week' ? `week?start=${from}` : `month?month=${date.slice(0, 7)}`
    $('cal-download').href = `/api/admin/poster/${poster}&download=1`
    let items
    try {
      items = (await api(`/api/admin/calendar?from=${from}&to=${to}`)).items
    } catch (e) {
      $('cal').innerHTML = `<p class="text-sm text-red-600">${esc(e.message)}</p>`
      return
    }
    const byDay = {}
    for (const r of items) (byDay[r.day] ||= []).push(r)
    $('cal').innerHTML = view === 'month' ? monthView(from, to, byDay) : view === 'week' ? weekView(from, byDay) : dayView(date, byDay[date] || [])
  }

  const totalHours = () => state.me.close - state.me.open
  const bookedHours = rows => rows.reduce((n, r) => n + r.hours, 0)

  function monthView(from, to, byDay) {
    const month = state.cal.date.slice(0, 7)
    const head = ['Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat', 'Sun'].map(d => `<div class="pb-2 text-center text-xs font-bold uppercase tracking-wider text-muted">${d}</div>`).join('')
    let cells = ''
    for (let d = from; d <= to; d = addDays(d, 1)) {
      const rows = byDay[d] || []
      const used = bookedHours(rows)
      const full = used >= totalHours()
      const inMonth = d.slice(0, 7) === month
      const pct = Math.round(used / totalHours() * 100)
      cells += `<button type="button" data-day="${d}" class="flex min-h-24 flex-col rounded-xl border p-2 text-left transition hover:border-dink/50 ${full ? 'border-dink bg-dink text-white' : 'border-line bg-white'} ${inMonth ? '' : 'opacity-40'} ${d === state.me.today ? 'ring-2 ring-skibi' : ''}">
        <span class="font-display text-lg font-bold">${+d.slice(8)}</span>
        ${full ? '<span class="mt-auto text-[11px] font-bold uppercase tracking-wide">Full</span>' : rows.length ? `<span class="mt-auto text-[11px] font-semibold text-muted">${rows.length} booking${rows.length === 1 ? '' : 's'} · ${used} h</span><span class="mt-1 h-1.5 overflow-hidden rounded-full bg-slate-100"><span class="block h-full rounded-full bg-dink" style="width:${pct}%"></span></span>` : ''}
      </button>`
    }
    return `<div class="grid grid-cols-7 gap-1.5 sm:gap-2">${head}${cells}</div>`
  }

  function weekView(from, byDay) {
    const { open, close } = state.me
    const rowH = 2.25
    let cols = ''
    for (let i = 0; i < 7; i++) {
      const d = addDays(from, i)
      const rows = byDay[d] || []
      const blocks = rows.map(r => `<div class="absolute inset-x-1 overflow-hidden rounded-md bg-dink px-1.5 py-1 text-[11px] leading-tight text-white" style="top:${(r.start_hour - open) * rowH}rem;height:${r.hours * rowH - 0.15}rem" title="${esc(r.name)} · ${span(r)}"><b>${esc(r.name)}</b><br>${span(r)}</div>`).join('')
      cols += `<div class="min-w-24 flex-1">
        <button type="button" data-day="${d}" class="mb-2 w-full text-center ${d === state.me.today ? 'text-skibi' : 'text-ink'}"><span class="block text-[11px] font-bold uppercase tracking-wider text-muted">${shortDate(d).split(' ')[0].replace(',', '')}</span><span class="font-display text-xl font-bold">${+d.slice(8)}</span>${bookedHours(rows) >= totalHours() ? '<span class="ml-1 rounded bg-dink px-1 text-[10px] font-bold text-white">FULL</span>' : ''}</button>
        <div class="relative rounded-lg border border-line bg-white" style="height:${(close - open) * rowH}rem;background-image:repeating-linear-gradient(to bottom,transparent 0 calc(${rowH}rem - 1px),#e2e8f0 calc(${rowH}rem - 1px) ${rowH}rem)">${blocks}</div>
      </div>`
    }
    let labels = ''
    for (let h = open; h < close; h++) labels += `<div class="pr-2 text-right text-[11px] text-muted" style="height:${rowH}rem">${fmtHour(h)}</div>`
    return `<div class="flex gap-1.5 overflow-x-auto pb-2"><div class="shrink-0 pt-[3.75rem]">${labels}</div>${cols}</div>`
  }

  function dayView(date, rows) {
    const { open, close } = state.me
    let out = ''
    for (let h = open; h < close; h++) {
      const r = rows.find(x => x.start_hour <= h && x.start_hour + x.hours > h)
      const first = r && r.start_hour === h
      out += `<div class="flex items-stretch gap-4 border-b border-line py-2 last:border-0">
        <span class="w-16 shrink-0 pt-2 text-right text-sm font-semibold text-muted">${fmtHour(h)}</span>
        ${!r ? '<span class="flex-1 rounded-lg border border-dashed border-slate-200 px-3 py-2 text-sm text-slate-400">Open</span>'
          : `<div class="flex-1 rounded-lg ${first ? 'bg-dink text-white' : 'bg-dink/80 text-white/90'} px-3 py-2 text-sm">${first ? `<b>${esc(r.name)}</b> · ${span(r)} · ${r.players} players${r.paid_at ? ' · paid' : ' · <b>not paid</b>'}<span class="mt-0.5 block text-xs text-white/80">${esc(r.phone)} ${esc(r.facebook)}${r.note ? ' · ' + esc(r.note) : ''}</span>` : `${esc(r.name)} (continued)`}</div>`}
      </div>`
    }
    const used = bookedHours(rows)
    return `<p class="mb-4 text-sm font-semibold ${used >= totalHours() ? 'text-dink' : 'text-skibi-dark'}">${used >= totalHours() ? 'Fully booked' : `${totalHours() - used} of ${totalHours()} hours open`}</p>
      <div class="rounded-2xl border border-line bg-white px-4">${out}</div>`
  }

  function moveCal(step) {
    const { view, date } = state.cal
    if (view === 'day') state.cal.date = addDays(date, step)
    else if (view === 'week') state.cal.date = addDays(date, 7 * step)
    else {
      const first = date.slice(0, 8) + '01'
      state.cal.date = step > 0 ? addDays(first, 32).slice(0, 8) + '01' : addDays(first, -1).slice(0, 8) + '01'
    }
    loadCalendar()
  }

  // Dashboard

  function bars(el, data, label, unit) {
    const max = Math.max(1, ...data.map(d => d.v))
    $(el).innerHTML = data.map(d => `<div class="bar-col" tabindex="0" aria-label="${esc(label(d))}: ${d.v} ${unit}">
      <span class="bar-tip">${esc(label(d))}: ${d.v} ${unit}</span>
      <span class="bar" style="height:${d.v / max * 100}%"></span>
      <span class="bar-label">${esc(d.short ?? label(d))}</span>
    </div>`).join('')
  }

  async function loadStats() {
    let s, open
    try {
      ({ stats: s, open } = await api('/api/admin/stats'))
    } catch (e) {
      $('tiles').innerHTML = `<p class="col-span-full text-sm text-red-600">${esc(e.message)}</p>`
      return
    }
    const tile = (label, value, note) => `<div class="rounded-2xl border border-line bg-white p-4"><p class="text-xs font-semibold uppercase tracking-wider text-muted">${label}</p><p class="mt-1 font-display text-3xl font-bold text-ink">${Number(value).toLocaleString('en-PH')}</p>${note ? `<p class="mt-1 text-xs text-muted">${note}</p>` : ''}</div>`
    $('tiles').innerHTML = [
      tile('Players so far', s.playersPlayed, 'People on court from confirmed bookings up to today'),
      tile('Customers', s.customers, `${s.returning} came back more than once`),
      tile('Confirmed bookings', s.confirmed, `${s.hours} court hours · ${s.upcoming} still ahead`),
      tile('Needs action', s.requested + s.awaiting, `${s.requested} requested · ${s.awaiting} awaiting payment`),
      tile('All requests', s.requests, s.requests ? `${Math.round(s.confirmed / s.requests * 100)}% became bookings` : ''),
      tile('Players booked', s.players, 'Including bookings still ahead'),
      tile('Paid online', s.online, ''),
      tile('Walk-ins', s.walkIns, 'Booked at the court'),
    ].join('')
    bars('chart-months', s.months.map(m => ({ v: m.players, short: dateOf(`${m.month}-01`).toLocaleDateString('en-PH', { month: 'short', timeZone: 'UTC' }), m })), d => monthName(`${d.m.month}-01`), 'players')
    bars('chart-hours', s.byHour.map((v, i) => ({ v, h: open + i, short: (open + i) % 3 === 0 ? fmtHour(open + i).replace(' ', '') : '' })), d => `${fmtHour(d.h)}–${fmtHour(d.h + 1)}`, 'bookings')
    bars('chart-days', s.byWeekday.map((v, i) => ({ v, d: ['Monday', 'Tuesday', 'Wednesday', 'Thursday', 'Friday', 'Saturday', 'Sunday'][i] })).map(x => ({ ...x, short: x.d.slice(0, 3) })), d => d.d, 'bookings')
  }

  // Wiring

  $('login-form').addEventListener('submit', async e => {
    e.preventDefault()
    $('login-error').textContent = ''
    try {
      await api('/api/admin/login', { password: $('password').value })
      $('password').value = ''
      start()
    } catch (err) {
      $('login-error').textContent = err.message
    }
  })
  $('logout').addEventListener('click', async () => {
    await api('/api/admin/logout', {}).catch(() => {})
    showLogin()
  })
  document.querySelectorAll('.admin-tab').forEach(b => b.addEventListener('click', () => show(b.dataset.tab)))
  $('search').addEventListener('input', () => { clearTimeout(searchTimer); searchTimer = setTimeout(loadList, 250) })
  $('scope').addEventListener('change', loadList)
  $('filters').addEventListener('click', e => {
    const b = e.target.closest('[data-filter]')
    if (!b) return
    state.filter = b.dataset.filter
    renderList()
  })
  $('list').addEventListener('click', e => {
    const b = e.target.closest('[data-act]')
    if (!b) return
    if (b.dataset.user) block(b.dataset.user, b.dataset.act === 'ban')
    else act(b.dataset.id, b.dataset.act)
  })

  async function block(userId, on) {
    const reason = on
      ? await ask('Block this account from booking? They see the reason on their account page.', { ok: 'Block account', danger: true, input: 'Reason, e.g. fake requests' })
      : await ask('Unblock this account? They can book again.', { ok: 'Unblock' })
    if (reason === false) return
    try {
      await api(`/api/admin/users/${userId}/ban`, { ban: on, reason: on ? reason : '' })
      toast(on ? 'Account blocked.' : 'Account unblocked.')
    } catch (e) {
      toast(e.message)
    }
    loadList()
  }
  $('walkin-open').addEventListener('click', () => {
    const f = $('walkin')
    f.classList.remove('hidden')
    f.classList.add('grid')
    const day = f.querySelector('[name=date]')
    if (!day.value) day.value = state.me.today
    f.querySelector('[name=name]').focus()
  })
  $('walkin-close').addEventListener('click', () => { $('walkin').classList.add('hidden'); $('walkin').classList.remove('grid') })
  $('walkin').addEventListener('submit', async e => {
    e.preventDefault()
    const f = e.target
    const body = Object.fromEntries(new FormData(f))
    body.paid = f.querySelector('[name=paid]').checked
    $('walkin-error').textContent = ''
    try {
      await api('/api/admin/reservations', body)
      f.reset()
      f.classList.add('hidden')
      f.classList.remove('grid')
      toast('Walk-in booking saved.')
      loadList()
    } catch (err) {
      $('walkin-error').textContent = err.message
    }
  })
  document.querySelectorAll('.cal-view').forEach(b => b.addEventListener('click', () => { state.cal.view = b.dataset.view; loadCalendar() }))
  $('cal-prev').addEventListener('click', () => moveCal(-1))
  $('cal-next').addEventListener('click', () => moveCal(1))
  $('cal-today').addEventListener('click', () => { state.cal.date = state.me.today; loadCalendar() })
  $('cal').addEventListener('click', e => {
    const b = e.target.closest('[data-day]')
    if (!b) return
    state.cal = { view: 'day', date: b.dataset.day }
    loadCalendar()
  })

  start()
})()
