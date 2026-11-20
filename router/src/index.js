import { cacheControl, isStatic, snapshotPath } from './assets.js'

async function fromAssets(request, env, url) {
  if (!env.ASSETS || (request.method !== 'GET' && request.method !== 'HEAD') || !isStatic(url.pathname)) return null
  const other = url.pathname.match(/^\/_v\/[0-9a-f]+(\/.+)$/)
  const v = url.searchParams.get('v')
  // DANGER: a /_v/ hash or ?v= from another deploy gets this deploy's file. Sent immutable, that
  // pins the URL in every browser to the wrong bytes for a year, so it must stay no-cache.
  const stale = !!other || (!!v && v !== env.ASSETS_VERSION)
  const path = other ? other[1] : url.pathname
  const res = await env.ASSETS.fetch(other ? new Request(new URL(path, url), request) : request)
  if (res.status === 404) return null
  const out = new Response(res.body, res)
  out.headers.set('cache-control', stale ? 'no-cache' : cacheControl(path))
  secure(out.headers)
  return out
}

async function snapshot(request, env, url, anyQuery) {
  if (!env.ASSETS || (request.method !== 'GET' && request.method !== 'HEAD')) return null
  const path = snapshotPath(url, anyQuery)
  if (!path) return null
  const res = await env.ASSETS.fetch(new Request(new URL(path, url), { method: request.method, headers: request.headers }))
  if (res.status !== 200 && res.status !== 304) return null
  const out = new Response(res.body, res)
  out.headers.set('content-type', 'text/html; charset=utf-8')
  out.headers.set('cache-control', 'public, max-age=0, must-revalidate')
  secure(out.headers)
  out.headers.set('x-skibidink-snapshot', env.ASSETS_VERSION || '')
  return out
}

function secure(headers) {
  headers.set('x-content-type-options', 'nosniff')
  headers.set('strict-transport-security', 'max-age=31536000')
  headers.set('referrer-policy', 'strict-origin-when-cross-origin')
  headers.set('x-frame-options', 'SAMEORIGIN')
}

function proxy(request, env, url) {
  if (!env.ORIGIN) return null
  const headers = new Headers(request.headers)
  headers.delete('x-skibidink-origin')
  if (env.EDGE_SECRET) headers.set('x-skibidink-edge', env.EDGE_SECRET)
  headers.set('x-skibidink-client-ip', request.headers.get('cf-connecting-ip') || '')
  headers.set('x-skibidink-host', url.host)
  return fetch(new URL(url.pathname + url.search, env.ORIGIN), {
    method: request.method,
    headers,
    body: request.body,
    redirect: 'manual',
  })
}

export default {
  async fetch(request, env) {
    const url = new URL(request.url)
    const asset = await fromAssets(request, env, url)
    if (asset) return asset

    const live = env.EDGE_SECRET && request.headers.get('x-skibidink-origin') === env.EDGE_SECRET
    if (!live) {
      const page = await snapshot(request, env, url, false)
      if (page) return page
    }

    let res = null
    try {
      res = await proxy(request, env, url)
    } catch {}
    if (!res || res.status >= 500) {
      const page = await snapshot(request, env, url, true)
      if (page) return page
    }
    if (!res && !env.ORIGIN && (request.method === 'GET' || request.method === 'HEAD')) return Response.redirect(new URL('/', url), 302)
    return res ?? new Response('The site is unreachable right now.', { status: 502 })
  },
}
