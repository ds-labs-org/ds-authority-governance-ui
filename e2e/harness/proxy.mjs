// Stand-in for the apisix gateway in front of the console:
//   /configuration.json -> generated runtime config (api-contract.md section 3)
//   /api/issuer/*       -> hub admin listener, prefix stripped, hub admin token injected
//   /api/registry/*     -> trusted issuers registry, prefix stripped, registry admin token injected
//   anything else       -> static files from the trunk-built dist/ (index.html for `/`)
//
// Paths are forwarded byte for byte (no decoding), so a DID sent as
// `did%3Aweb%3Ahost%253A8443` reaches the registry unchanged.
//
// Standalone: `UI_DIST=../dist HUB_ADMIN_URL=... HUB_ADMIN_TOKEN=... \
//   REGISTRY_URL=... REGISTRY_ADMIN_TOKEN=... node harness/proxy.mjs [port]`

import http from 'node:http';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const HERE = path.dirname(fileURLToPath(import.meta.url));
export const DEFAULT_DIST = path.resolve(HERE, '..', '..', 'dist');

const MIME = {
  '.html': 'text/html; charset=utf-8',
  '.js': 'text/javascript; charset=utf-8',
  '.mjs': 'text/javascript; charset=utf-8',
  '.wasm': 'application/wasm',
  '.css': 'text/css; charset=utf-8',
  '.json': 'application/json',
  '.svg': 'image/svg+xml',
  '.png': 'image/png',
  '.ico': 'image/x-icon',
  '.woff': 'font/woff',
  '.woff2': 'font/woff2',
  '.ttf': 'font/ttf',
};

// Hop-by-hop headers never forwarded (RFC 9110 7.6.1), plus `host`.
const HOP = new Set([
  'connection',
  'keep-alive',
  'proxy-authenticate',
  'proxy-authorization',
  'te',
  'trailer',
  'transfer-encoding',
  'upgrade',
  'host',
]);

export function defaultUiConfig(overrides = {}) {
  return {
    identity_api_path: '/api/identity',
    issuer_admin_api_path: '/api/issuer',
    registry_api_path: '/api/registry',
    participant_context_id: 'issuer',
    bearer_token: null,
    ...overrides,
  };
}

function forward(req, res, targetBase, token, strippedPath) {
  const target = new URL(targetBase);
  const headers = {};
  for (const [k, v] of Object.entries(req.headers)) if (!HOP.has(k)) headers[k] = v;
  // The gateway authenticates the operator (OIDC cookie) and calls the
  // backend with its own service credential - any browser-supplied
  // Authorization header is replaced, never passed through.
  delete headers.authorization;
  delete headers.cookie;
  if (token) headers.authorization = `Bearer ${token}`;
  const basePath = target.pathname.replace(/\/$/, '');
  const upstream = http.request(
    {
      host: target.hostname,
      port: target.port,
      method: req.method,
      path: `${basePath}${strippedPath || '/'}`,
      headers,
    },
    (up) => {
      const out = {};
      for (const [k, v] of Object.entries(up.headers)) if (!HOP.has(k)) out[k] = v;
      res.writeHead(up.statusCode ?? 502, out);
      up.pipe(res);
    },
  );
  upstream.on('error', (err) => {
    if (!res.headersSent) {
      res.writeHead(502, { 'content-type': 'application/json' });
    }
    res.end(JSON.stringify({ error: `gateway: upstream ${targetBase} unreachable: ${err.message}` }));
  });
  req.pipe(upstream);
}

function serveStatic(req, res, distDir) {
  const rawPath = decodeURIComponent(new URL(req.url, 'http://x').pathname);
  const rel = rawPath === '/' ? 'index.html' : rawPath.replace(/^\/+/, '');
  const file = path.resolve(distDir, rel);
  if (!file.startsWith(path.resolve(distDir) + path.sep)) {
    res.writeHead(403).end();
    return;
  }
  fs.stat(file, (err, st) => {
    if (err || !st.isFile()) {
      if (rel === 'index.html') {
        res.writeHead(503, { 'content-type': 'text/plain' });
        res.end(`dist/ not built: ${distDir}/index.html missing. Run \`trunk build\` in the repo root.`);
        return;
      }
      res.writeHead(404, { 'content-type': 'text/plain' }).end('not found');
      return;
    }
    res.writeHead(200, {
      'content-type': MIME[path.extname(file)] ?? 'application/octet-stream',
      'cache-control': 'no-store',
    });
    fs.createReadStream(file).pipe(res);
  });
}

/**
 * Starts the gateway. Options:
 *   distDir, uiConfig (object written as /configuration.json),
 *   hubAdminUrl, hubAdminToken, registryUrl, registryAdminToken,
 *   identityApiUrl (optional), host, port.
 * Returns { url, port, close() }.
 */
export async function startProxy({
  distDir = DEFAULT_DIST,
  uiConfig = defaultUiConfig(),
  hubAdminUrl,
  hubAdminToken,
  registryUrl,
  registryAdminToken,
  identityApiUrl,
  host = '127.0.0.1',
  port = 0,
} = {}) {
  const routes = [
    ['/api/issuer', hubAdminUrl, hubAdminToken],
    ['/api/registry', registryUrl, registryAdminToken],
    ['/api/identity', identityApiUrl, null],
  ];
  const server = http.createServer((req, res) => {
    const pathOnly = req.url.split('?')[0];
    if (pathOnly === '/configuration.json') {
      res.writeHead(200, { 'content-type': 'application/json', 'cache-control': 'no-store' });
      res.end(JSON.stringify(uiConfig, null, 2));
      return;
    }
    for (const [prefix, target, token] of routes) {
      if (req.url === prefix || req.url.startsWith(`${prefix}/`) || req.url.startsWith(`${prefix}?`)) {
        if (!target) {
          res.writeHead(502, { 'content-type': 'application/json' });
          res.end(JSON.stringify({ error: `gateway: no backend configured for ${prefix}` }));
          return;
        }
        forward(req, res, target, token, req.url.slice(prefix.length));
        return;
      }
    }
    if (req.method !== 'GET' && req.method !== 'HEAD') {
      res.writeHead(405).end();
      return;
    }
    serveStatic(req, res, distDir);
  });
  await new Promise((resolve, reject) => {
    server.once('error', reject);
    server.listen(port, host, resolve);
  });
  const actualPort = server.address().port;
  return {
    url: `http://${host}:${actualPort}`,
    port: actualPort,
    close: () =>
      new Promise((resolve) => {
        server.closeAllConnections?.();
        server.close(() => resolve());
      }),
  };
}

if (import.meta.url === pathToFileURL(process.argv[1] ?? '').href) {
  const p = await startProxy({
    distDir: process.env.UI_DIST ? path.resolve(process.env.UI_DIST) : DEFAULT_DIST,
    hubAdminUrl: process.env.HUB_ADMIN_URL,
    hubAdminToken: process.env.HUB_ADMIN_TOKEN,
    registryUrl: process.env.REGISTRY_URL,
    registryAdminToken: process.env.REGISTRY_ADMIN_TOKEN,
    port: Number(process.argv[2] ?? 8080),
  });
  console.log(`gateway on ${p.url}`);
}
