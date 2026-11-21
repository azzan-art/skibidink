(() => {
  const $ = id => document.getElementById(id)
  const esc = s => String(s ?? '').replace(/[&<>"']/g, c => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' })[c])
  const fmtHour = h => `${h % 12 || 12} ${h < 12 || h === 24 ? 'AM' : 'PM'}`
  const longDate = iso => new Date(`${iso}T12:00:00Z`).toLocaleDateString('en-PH', { weekday: 'short', month: 'short', day: 'numeric', year: 'numeric', timeZone: 'UTC' })
  const STATUS = { requested: 'Requested', awaiting: 'Awaiting payment', confirmed: 'Confirmed', declined: 'Declined', cancelled: 'Cancelled' }
  const q = new URLSearchParams(location.search)
  const next = (() => {
    const n = q.get('next') || ''
    return n.startsWith('/') && !n.startsWith('//') ? n : ''
  })()

  async function api(path, body) {
    const res = await fetch(path, body === undefined ? {} : { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(body) })
    const data = await res.json().catch(() => ({}))
    if (!res.ok) throw Object.assign(new Error(data.error || 'Something went wrong. Try again.'), { data, status: res.status })
    return data
  }

  const form = el => Object.fromEntries(new FormData(el))

  function flash(text, good = true) {
    const el = $('acct-flash')
    el.textContent = text
    el.className = `mb-4 rounded-xl px-4 py-3 text-sm font-semibold ${good ? 'bg-skibi/10 text-skibi-dark' : 'bg-red-50 text-red-700'}`
  }

  function view(name) {
    for (const v of ['auth', 'reset', 'sent', 'me']) $(`view-${v}`).classList.toggle('hidden', v !== name)
  }

  function authForm(which) {
    view('auth')
    for (const f of ['login', 'register', 'forgot']) {
      $(`form-${f}`).classList.toggle('hidden', f !== which)
      $(`form-${f}`).classList.toggle('grid', f === which)
    }
    document.querySelectorAll('.auth-tab').forEach(b => {
      const on = b.dataset.auth === which
      b.setAttribute('aria-selected', on)
      b.classList.toggle('bg-white', on)
      b.classList.toggle('shadow-sm', on)
      b.classList.toggle('text-muted', !on)
    })
    $('auth-error').textContent = ''
    $('resend').classList.add('hidden')
  }

  function sent(text) {
    $('sent-text').textContent = text
    view('sent')
  }

  async function showMe() {
    let me
    try {
      me = (await api('/api/account/me')).user
    } catch {
      return false
    }
    if (next) {
      location.href = sitePath(next)
      return true
    }
    $('me-name').textContent = me.name.split(' ')[0]
    $('me-banned').classList.toggle('hidden', !me.banned)
    $('me-banned-reason').textContent = me.banReason ? `Reason: ${me.banReason}` : ''
    $('me-book').classList.toggle('hidden', !!me.banned)
    $('me-email').textContent = me.email
    const p = $('form-profile')
    p.querySelector('[name=name]').value = me.name
    p.querySelector('[name=phone]').value = formatPhone(me.phone)
    p.querySelector('[name=facebook]').value = me.facebook
    view('me')
    try {
      const { items } = await api('/api/account/reservations')
      renderBookings(items)
    } catch {
      for (const id of ['my-booked', 'my-requests', 'my-past']) $(id).innerHTML = '<li class="text-muted">Could not load your bookings.</li>'
    }
    return true
  }

  function renderBookings(items) {
    const today = new Intl.DateTimeFormat('en-CA', { timeZone: 'Asia/Manila' }).format(new Date())
    const open = r => r.status === 'requested' || r.status === 'awaiting'
    const ahead = r => r.day >= today
    const asc = (a, b) => a.day.localeCompare(b.day) || a.start_hour - b.start_hour
    const row = r => {
      const taken = r.taken_at && open(r)
      const pill = taken
        ? '<span class="status status-taken ml-auto" title="Someone else booked and paid for these hours first">Taken</span>'
        : `<span class="status status-${esc(r.status)} ml-auto">${STATUS[r.status] || esc(r.status)}</span>`
      return `<li class="flex flex-wrap items-center gap-x-3 gap-y-1 rounded-xl border ${taken ? 'border-rose-200 bg-rose-50/50' : 'border-line'} px-3 py-2">
        <span class="font-semibold text-ink">${longDate(r.day)}</span>
        <span class="text-muted">${fmtHour(r.start_hour)}–${fmtHour(r.start_hour + r.hours)} · ${r.players} players</span>
        ${pill}
        ${taken ? `<span class="basis-full text-xs text-rose-700">Someone else paid for these hours first. <a href="${sitePath('/book')}" class="font-semibold underline">Pick another time</a></span>` : ''}
      </li>`
    }
    const fill = (id, rows, empty) => { $(id).innerHTML = rows.length ? rows.map(row).join('') : `<li class="text-muted">${empty}</li>` }
    fill('my-booked', items.filter(r => r.status === 'confirmed' && ahead(r)).sort(asc), 'Nothing booked yet.')
    fill('my-requests', items.filter(r => open(r) && ahead(r)).sort(asc), 'No open requests.')
    fill('my-past', items.filter(r => !ahead(r) || !(open(r) || r.status === 'confirmed')), 'Nothing here yet.')
  }

  async function submit(el, errorEl, run) {
    const button = el.querySelector('[type=submit]')
    button.disabled = true
    $(errorEl).textContent = ''
    try {
      await run(form(el))
    } catch (err) {
      $(errorEl).textContent = err.message
      if (err.data?.unverified) $('resend').classList.remove('hidden')
    } finally {
      button.disabled = false
    }
  }

  $('form-login').addEventListener('submit', e => {
    e.preventDefault()
    submit(e.target, 'auth-error', async body => {
      await api('/api/account/login', body)
      await showMe()
    })
  })
  $('form-register').addEventListener('submit', e => {
    e.preventDefault()
    submit(e.target, 'auth-error', async body => {
      const res = await api('/api/account/register', body)
      if (res.demo) return showMe()
      sent(`We sent a confirmation link to ${body.email}. Open it to finish your account, then you can book. Check spam if it isn't there in a minute.`)
    })
  })
  $('form-forgot').addEventListener('submit', e => {
    e.preventDefault()
    submit(e.target, 'auth-error', async body => {
      await api('/api/account/forgot', body)
      sent(`If ${body.email} has an account, a reset link is on its way. It works for 1 hour.`)
    })
  })
  $('form-reset').addEventListener('submit', e => {
    e.preventDefault()
    submit(e.target, 'reset-error', async body => {
      await api('/api/account/reset', { ...body, token: q.get('reset') })
      history.replaceState(null, '', location.pathname)
      flash('Password changed. You are signed in.')
      await showMe()
    })
  })
  $('form-profile').addEventListener('submit', async e => {
    e.preventDefault()
    const msg = $('profile-msg')
    try {
      await api('/api/account/profile', form(e.target))
      msg.textContent = 'Saved.'
      msg.className = 'min-h-5 text-sm text-skibi-dark'
    } catch (err) {
      msg.textContent = err.message
      msg.className = 'min-h-5 text-sm text-red-600'
    }
  })
  $('resend').addEventListener('click', async () => {
    const email = $('form-login').querySelector('[name=email]').value
    await api('/api/account/resend', { email }).catch(() => {})
    sent(`We sent a new confirmation link to ${email}.`)
  })
  $('to-forgot').addEventListener('click', () => authForm('forgot'))
  document.querySelectorAll('.back-login').forEach(b => b.addEventListener('click', () => authForm('login')))
  document.querySelectorAll('.auth-tab').forEach(b => b.addEventListener('click', () => authForm(b.dataset.auth)))
  $('signout').addEventListener('click', async () => {
    await api('/api/account/logout', {}).catch(() => {})
    authForm('login')
  })

  async function start() {
    if (q.has('reset')) return view('reset')
    if (q.get('verified')) flash('Email confirmed. You are signed in and can book now.')
    if (q.get('verify') === 'expired') flash('That confirmation link has expired or was already used. Sign in, or ask for a new link.', false)
    if (q.has('verified') || q.has('verify')) history.replaceState(null, '', location.pathname + (next ? `?next=${encodeURIComponent(next)}` : ''))
    if (await showMe()) return
    authForm(q.get('mode') === 'register' ? 'register' : 'login')
    if (next) flash('Sign in or create an account to book the court.')
  }

  start()
})()
