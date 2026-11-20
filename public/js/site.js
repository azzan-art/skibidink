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
