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

const navLogo = document.getElementById('nav-logo')
const heroLogo = document.getElementById('hero-logo')
if (navLogo && heroLogo) {
  new IntersectionObserver(([e]) => navLogo.classList.toggle('show', !e.isIntersecting), { rootMargin: '-72px 0px 0px 0px' }).observe(heroLogo)
} else if (navLogo) {
  navLogo.classList.add('show')
}
