const seen = new IntersectionObserver(entries => {
  for (const e of entries) {
    if (e.isIntersecting) {
      e.target.classList.add('in')
      seen.unobserve(e.target)
    }
  }
}, { rootMargin: '0px 0px -8% 0px' })
document.querySelectorAll('.reveal').forEach(el => seen.observe(el))

const year = document.getElementById('year')
if (year) year.textContent = new Date().getFullYear()

const reveal = document.querySelectorAll('.nav-reveal')
const heroLogo = document.getElementById('hero-logo')
const showNav = on => reveal.forEach(el => el.classList.toggle('show', on))
if (heroLogo) {
  new IntersectionObserver(([e]) => showNav(!e.isIntersecting), { rootMargin: '-72px 0px 0px 0px' }).observe(heroLogo)
} else {
  showNav(true)
}

const topbar = document.getElementById('nav')
if (topbar) {
  let queued = false
  const tint = () => {
    queued = false
    const depth = Math.min(scrollY / (innerHeight * 1.5), 1)
    topbar.style.setProperty('--tint', (depth * 0.72).toFixed(3))
  }
  addEventListener('scroll', () => {
    if (!queued) {
      queued = true
      requestAnimationFrame(tint)
    }
  }, { passive: true })
  tint()
}

const links = [...document.querySelectorAll('.topbar-links a[href^="#"]')]
const spied = links.map(a => document.querySelector(a.getAttribute('href'))).filter(Boolean)
if (spied.length) {
  const spy = new IntersectionObserver(entries => {
    for (const e of entries) {
      if (!e.isIntersecting) continue
      links.forEach(a => a.classList.toggle('is-active', a.getAttribute('href') === '#' + e.target.id))
    }
  }, { rootMargin: '-45% 0px -54% 0px' })
  spied.forEach(el => spy.observe(el))
  const clear = new IntersectionObserver(entries => {
    if (entries.some(e => e.isIntersecting)) links.forEach(a => a.classList.remove('is-active'))
  }, { rootMargin: '-45% 0px -54% 0px' })
  document.querySelectorAll('main > section:not([id]), #book, #join').forEach(el => clear.observe(el))
}

window.sitePath = p => (window.SITE_BASE || '') + p

window.ask = (text, { ok = 'Confirm', cancel = 'Keep it', danger = false, input = null } = {}) => new Promise(resolve => {
  const d = document.createElement('dialog')
  d.className = 'ask'
  d.innerHTML = `<p class="ask-text"></p>${input === null ? '' : '<input class="field mt-4" maxlength="200">'}<div class="ask-actions"><button type="button" class="btn-ghost" value="0"></button><button type="button" class="${danger ? 'btn-danger' : 'btn-primary'}" value="1"></button></div>`
  d.querySelector('.ask-text').textContent = text
  const [no, yes] = d.querySelectorAll('button')
  no.textContent = cancel
  yes.textContent = ok
  const field = d.querySelector('input')
  if (field) field.placeholder = input
  const done = v => { d.close(); d.remove(); resolve(v && field ? field.value.trim() : v) }
  no.onclick = () => done(false)
  yes.onclick = () => done(true)
  field?.addEventListener('keydown', e => { if (e.key === 'Enter') done(true) })
  d.addEventListener('cancel', e => { e.preventDefault(); done(false) })
  d.addEventListener('click', e => { if (e.target === d) done(false) })
  document.body.append(d)
  d.showModal()
  ;(field || yes).focus()
})

function formatPhone(value) {
  const d = value.replace(/\D/g, '')
  const group = (digits, sizes) => {
    const out = []
    for (const n of sizes) if (digits) out.push(digits.slice(0, n)), digits = digits.slice(n)
    return out.join(' ') + (digits ? ' ' + digits : '')
  }
  if (value.trim().startsWith('+') || d.startsWith('63')) {
    if (!d.startsWith('63')) return '+' + value.replace(/[^\d ]/g, '').replace(/ +/g, ' ').trimStart()
    return ('+63 ' + group(d.slice(2, 12), [3, 3, 4])).trim()
  }
  return group(d.slice(0, 11), [4, 3, 4])
}

function reformatPhone(input) {
  const caret = input.selectionStart ?? input.value.length
  const digitsBefore = input.value.slice(0, caret).replace(/[^\d+]/g, '').length
  const next = formatPhone(input.value)
  input.value = next
  let pos = 0
  for (let seen = 0; pos < next.length && seen < digitsBefore; pos++) if (/[\d+]/.test(next[pos])) seen++
  input.setSelectionRange(pos, pos)
}

document.addEventListener('beforeinput', e => {
  const input = e.target
  if (!input.matches?.('input[name=phone]') || e.inputType !== 'deleteContentBackward') return
  const at = input.selectionStart
  if (at !== input.selectionEnd || input.value[at - 1] !== ' ') return
  e.preventDefault()
  input.value = input.value.slice(0, at - 2) + input.value.slice(at - 1)
  input.setSelectionRange(at - 2, at - 2)
  reformatPhone(input)
})
document.addEventListener('input', e => {
  if (e.target.matches?.('input[name=phone]')) reformatPhone(e.target)
})
