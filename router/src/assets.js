// Bundled into the router Worker, so it must stay free of Node APIs and of imports.

// DANGER: must stay the same list as TYPES in server-rs/src/public.rs. An extension missing here
// sends that file to the origin; one added here serves a file the server would refuse.
const STATIC = new Set([
  'js', 'mjs', 'css', 'json', 'webmanifest', 'map', 'md', 'png', 'webp', 'jpg', 'jpeg', 'gif',
  'ico', 'svg', 'woff2', 'woff', 'otf', 'ttf', 'pdf', 'txt', 'xml',
])

export const FALLBACK = '/_fallback'

export function isStatic(pathname) {
  if (pathname.startsWith(FALLBACK + '/')) return false
  const segs = pathname.slice(1).split('/')
  if (segs.some(s => !s || s.startsWith('.'))) return false
  const dot = pathname.lastIndexOf('.')
  return dot > pathname.lastIndexOf('/') && STATIC.has(pathname.slice(dot + 1).toLowerCase())
}

export function cacheControl(pathname) {
  if (pathname.endsWith('sw.js')) return 'no-cache'
  return 'public, max-age=31536000, immutable'
}

// DANGER: a snapshot is one rendering per path. A key added here that changes the page hands every
// visitor the plain page instead.
export function plainPageQuery(url) {
  for (const key of url.searchParams.keys()) {
    if (!(key.startsWith('utm_') || key === 'fbclid' || key === 'gclid')) return false
  }
  return true
}

export function snapshotPath(url, anyQuery) {
  return anyQuery || plainPageQuery(url) ? fallbackPath(url.pathname) : null
}

export function fallbackPath(pathname) {
  const clean = pathname.replace(/\/+$/, '')
  return `${FALLBACK}${clean || '/index'}.html`
}
