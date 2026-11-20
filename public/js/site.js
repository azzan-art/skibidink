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
const darkBands = document.querySelectorAll('[data-dark]')
if (topbar && darkBands.length) {
  const under = new Set()
  const watch = new IntersectionObserver(entries => {
    for (const e of entries) e.isIntersecting ? under.add(e.target) : under.delete(e.target)
    topbar.classList.toggle('on-dark', under.size > 0)
  }, { rootMargin: '0px 0px -92% 0px' })
  darkBands.forEach(el => watch.observe(el))
}
