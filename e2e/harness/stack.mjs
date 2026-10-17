// Starts the whole e2e stack on free 127.0.0.1 ports:
//   hub (identity-hub issuer-service, manual holder approval, admin listener)
//   registry (trusted-issuers-registry, data file in .stack/, restartable)
//   fake DCP holder (./fake-holder.mjs)
//   gateway proxy serving dist/ (./proxy.mjs)
// and writes everything tests need to `.stack/stack.json` (see readStack).
// A small control server (stack.harnessUrl) lets specs restart the registry
// from its data file: `POST /registry/restart`.
//
// Env:
//   HUB_BIN          default <repo>/../ds-identity-hub-rs/target/debug/identity-hub
//   REGISTRY_BIN     required unless REGISTRY_URL is set
//   UI_DIST          default <repo>/dist
//   HUB_URL + HUB_ADMIN_URL + HUB_ADMIN_TOKEN       reuse a running hub instead
//     (+ HUB_STS_CLIENT_ID + HUB_STS_CLIENT_SECRET for its operator credentials)
//   REGISTRY_URL + REGISTRY_ADMIN_TOKEN             reuse a running registry instead
//   HUB_EXTRA_ARGS   extra whitespace-separated args for the hub
//   E2E_STACK_DIR    default <e2e>/.stack (state file + backend logs)
//
// Standalone: `node harness/stack.mjs` starts it and waits for Ctrl-C.

import { spawn } from 'node:child_process';
import fs from 'node:fs';
import http from 'node:http';
import net from 'node:net';
import path from 'node:path';
import { randomBytes } from 'node:crypto';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { startFakeHolder } from './fake-holder.mjs';
import { startProxy, defaultUiConfig, DEFAULT_DIST } from './proxy.mjs';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const E2E_DIR = path.resolve(HERE, '..');
const REPO_ROOT = path.resolve(E2E_DIR, '..');
export const STACK_DIR = process.env.E2E_STACK_DIR
  ? path.resolve(process.env.E2E_STACK_DIR)
  : path.join(E2E_DIR, '.stack');
export const STACK_FILE = path.join(STACK_DIR, 'stack.json');
export const PARTICIPANT_CONTEXT_ID = 'issuer';

const DEFAULT_HUB_BIN = path.resolve(REPO_ROOT, '..', 'ds-identity-hub-rs', 'target', 'debug', 'identity-hub');

export async function freePort() {
  return new Promise((resolve, reject) => {
    const s = net.createServer();
    s.unref();
    s.once('error', reject);
    s.listen(0, '127.0.0.1', () => {
      const { port } = s.address();
      s.close(() => resolve(port));
    });
  });
}

const token = () => randomBytes(24).toString('base64url');
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

function spawnLogged(name, bin, args, env) {
  if (!fs.existsSync(bin)) {
    throw new Error(`${name} binary not found at ${bin} - build it or set the env var (see e2e/README.md)`);
  }
  fs.mkdirSync(STACK_DIR, { recursive: true });
  const logFile = path.join(STACK_DIR, `${name}.log`);
  const out = fs.openSync(logFile, 'w');
  fs.writeSync(out, `$ ${bin} ${args.join(' ')}\n`);
  const child = spawn(bin, args, {
    env: { RUST_LOG: 'info', ...process.env, ...env },
    stdio: ['ignore', out, out],
  });
  fs.closeSync(out);
  const proc = { name, child, logFile, exited: null };
  child.on('exit', (code, signal) => {
    proc.exited = { code, signal };
  });
  return proc;
}

const tail = (file, n = 30) => {
  try {
    return fs.readFileSync(file, 'utf8').split('\n').slice(-n).join('\n');
  } catch {
    return '';
  }
};

async function waitFor(desc, check, { proc, timeoutMs = 30_000 } = {}) {
  const deadline = Date.now() + timeoutMs;
  let last = '';
  while (Date.now() < deadline) {
    if (proc?.exited) {
      throw new Error(
        `${proc.name} exited (${JSON.stringify(proc.exited)}) before ${desc}. Log tail (${proc.logFile}):\n${tail(proc.logFile)}`,
      );
    }
    try {
      if (await check()) return;
    } catch (err) {
      last = String(err?.message ?? err);
    }
    await sleep(150);
  }
  throw new Error(
    `timed out waiting for ${desc}${last ? ` (last error: ${last})` : ''}${proc ? `\nLog tail (${proc.logFile}):\n${tail(proc.logFile)}` : ''}`,
  );
}

/** base64 of the participant context id, as EDC puts it in admin paths. */
export const pcPath = (id = PARTICIPANT_CONTEXT_ID) => Buffer.from(id).toString('base64');

async function startHub() {
  if (process.env.HUB_URL) {
    if (!process.env.HUB_ADMIN_URL || !process.env.HUB_ADMIN_TOKEN) {
      throw new Error('HUB_URL needs HUB_ADMIN_URL and HUB_ADMIN_TOKEN too');
    }
    return {
      url: process.env.HUB_URL,
      adminUrl: process.env.HUB_ADMIN_URL,
      adminToken: process.env.HUB_ADMIN_TOKEN,
      stsClientId: process.env.HUB_STS_CLIENT_ID,
      stsClientSecret: process.env.HUB_STS_CLIENT_SECRET,
    };
  }
  const bin = process.env.HUB_BIN ? path.resolve(process.env.HUB_BIN) : DEFAULT_HUB_BIN;
  const [port, adminPort] = [await freePort(), await freePort()];
  const adminToken = token();
  // Operator credentials for the embedded STS and the OID4VCI operator
  // endpoints: never the hub's TCK defaults.
  const stsClientId = 'e2e-operator';
  const stsClientSecret = token();
  const args = [
    'issuer-service',
    '--bind', `127.0.0.1:${port}`,
    '--did-host', `127.0.0.1:${port}`,
    '--sts-client-id', stsClientId,
    '--sts-client-secret', stsClientSecret,
    // clap ArgAction::Set: the flag takes a value (it already defaults to true).
    '--insecure-http', 'true',
    '--admin-bind', `127.0.0.1:${adminPort}`,
    '--admin-token', adminToken,
    '--holder-approval', 'manual',
    ...(process.env.HUB_EXTRA_ARGS?.split(/\s+/).filter(Boolean) ?? []),
  ];
  const proc = spawnLogged('hub', bin, args, {});
  const url = `http://127.0.0.1:${port}`;
  const adminUrl = `http://127.0.0.1:${adminPort}`;
  await waitFor('hub public API (GET /metadata)', async () => (await fetch(`${url}/metadata`)).ok, { proc });
  await waitFor(
    'hub admin API (GET /v1alpha/participants/{pc})',
    async () =>
      (
        await fetch(`${adminUrl}/v1alpha/participants/${pcPath()}`, {
          headers: { authorization: `Bearer ${adminToken}` },
        })
      ).ok,
    { proc, timeoutMs: 15_000 },
  );
  return { url, adminUrl, adminToken, stsClientId, stsClientSecret, bin, proc };
}

async function startRegistry() {
  if (process.env.REGISTRY_URL) {
    return { url: process.env.REGISTRY_URL, adminToken: process.env.REGISTRY_ADMIN_TOKEN ?? '' };
  }
  if (!process.env.REGISTRY_BIN) {
    throw new Error(
      'REGISTRY_BIN is not set: point it at a built trusted-issuers-registry binary (ds-trusted-issuers-registry-rs, ' +
        '`cargo build -p trusted-issuers-http`), or set REGISTRY_URL + REGISTRY_ADMIN_TOKEN',
    );
  }
  const port = await freePort();
  const adminToken = token();
  const bin = path.resolve(process.env.REGISTRY_BIN);
  fs.mkdirSync(STACK_DIR, { recursive: true });
  const dataFile = path.join(STACK_DIR, 'registry.json');
  fs.rmSync(dataFile, { force: true });
  const args = [
    '--bind', `127.0.0.1:${port}`,
    // The registry only accepts a DNS name in --did-host (no IP literal),
    // so its did:web is `did:web:localhost%3A<port>`; it still binds 127.0.0.1.
    '--did-host', `localhost:${port}`,
    '--data-file', dataFile,
    '--create-data-file',
  ];
  const url = `http://127.0.0.1:${port}`;
  const registry = { url, adminToken, bin, dataFile, proc: null };
  const launch = async (name) => {
    registry.proc = spawnLogged(name, bin, args, { TRUSTED_ISSUERS_ADMIN_TOKEN: adminToken });
    await waitFor('registry (GET /health)', async () => (await fetch(`${url}/health`)).ok, {
      proc: registry.proc,
    });
  };
  await launch('registry');
  let restarts = 0;
  /** Stops the registry and starts it again from its data file, on the same
   * port and token. Rejects (with the log tail) if it does not come back. */
  registry.restart = async () => {
    await killProc(registry.proc);
    restarts += 1;
    await launch(`registry-restart-${restarts}`);
  };
  return registry;
}

/** The harness control server: what specs cannot do from a worker. */
async function startControl({ registry, onChange }) {
  const server = http.createServer(async (req, res) => {
    const reply = (status, body) => {
      res.writeHead(status, { 'content-type': 'application/json' });
      res.end(JSON.stringify(body));
    };
    if (req.method === 'POST' && req.url === '/registry/restart') {
      if (!registry.restart) return reply(501, { error: 'the registry was not started by the harness' });
      try {
        await registry.restart();
        onChange();
        return reply(200, { ok: true });
      } catch (err) {
        return reply(500, { error: String(err?.message ?? err) });
      }
    }
    return reply(404, { error: 'not found' });
  });
  await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
  return {
    url: `http://127.0.0.1:${server.address().port}`,
    close: () => new Promise((resolve) => server.close(() => resolve())),
  };
}

/** Starts everything. Returns { info, stop() }; `info` is also written to STACK_FILE. */
export async function startStack({ log = () => {} } = {}) {
  const started = [];
  const stop = async () => {
    for (const s of started.reverse()) {
      try {
        await s();
      } catch {
        /* best effort */
      }
    }
    started.length = 0;
  };
  try {
    const hub = await startHub();
    if (hub.proc) started.push(() => killProc(hub.proc));
    log(`hub ${hub.url} (admin ${hub.adminUrl})`);

    const registry = await startRegistry();
    if (registry.proc) started.push(() => killProc(registry.proc));
    log(`registry ${registry.url}`);

    let writeInfo = () => {};
    const control = await startControl({ registry, onChange: () => writeInfo() });
    started.push(() => control.close());

    const holder = await startFakeHolder();
    started.push(() => holder.close());
    log(`fake holder ${holder.origin}`);

    const distDir = process.env.UI_DIST ? path.resolve(E2E_DIR, process.env.UI_DIST) : DEFAULT_DIST;
    const uiConfig = defaultUiConfig({ participant_context_id: PARTICIPANT_CONTEXT_ID });
    const proxy = await startProxy({
      distDir,
      uiConfig,
      hubAdminUrl: hub.adminUrl,
      hubAdminToken: hub.adminToken,
      registryUrl: registry.url,
      registryAdminToken: registry.adminToken,
    });
    started.push(() => proxy.close());
    log(`console ${proxy.url} (dist ${distDir})`);

    const issuerDid = (await (await fetch(`${hub.url}/metadata`)).json()).issuer;
    const info = {
      uiUrl: proxy.url,
      harnessUrl: control.url,
      distDir,
      uiConfig,
      hub: {
        url: hub.url,
        did: issuerDid,
        adminUrl: hub.adminUrl,
        adminToken: hub.adminToken,
        stsClientId: hub.stsClientId ?? null,
        stsClientSecret: hub.stsClientSecret ?? null,
        bin: hub.bin ?? null,
        participantContextId: PARTICIPANT_CONTEXT_ID,
        log: hub.proc?.logFile ?? null,
      },
      registry: {
        url: registry.url,
        adminToken: registry.adminToken,
        bin: registry.bin ?? null,
        log: registry.proc?.logFile ?? null,
      },
      fakeHolder: { url: holder.origin, defaultDid: holder.holder().did },
      pids: [hub.proc?.child.pid, registry.proc?.child.pid].filter(Boolean),
    };
    writeInfo = () => {
      info.registry.log = registry.proc?.logFile ?? null;
      info.pids = [hub.proc?.child.pid, registry.proc?.child.pid].filter(Boolean);
      fs.mkdirSync(STACK_DIR, { recursive: true });
      fs.writeFileSync(STACK_FILE, JSON.stringify(info, null, 2));
    };
    writeInfo();
    return { info, stop };
  } catch (err) {
    await stop();
    throw err;
  }
}

function killProc(proc) {
  return new Promise((resolve) => {
    if (proc.exited) return resolve();
    const timer = setTimeout(() => {
      proc.child.kill('SIGKILL');
      resolve();
    }, 3000);
    proc.child.once('exit', () => {
      clearTimeout(timer);
      resolve();
    });
    proc.child.kill('SIGTERM');
  });
}

/** Reads the state written by startStack (from a Playwright worker). */
export function readStack() {
  if (!fs.existsSync(STACK_FILE)) {
    throw new Error(`${STACK_FILE} missing: the stack is started by Playwright globalSetup (npm test)`);
  }
  return JSON.parse(fs.readFileSync(STACK_FILE, 'utf8'));
}

if (import.meta.url === pathToFileURL(process.argv[1] ?? '').href) {
  const { info, stop } = await startStack({ log: console.log });
  console.log(JSON.stringify(info, null, 2));
  console.log('stack up - Ctrl-C to stop');
  const shutdown = async () => {
    await stop();
    process.exit(0);
  };
  process.on('SIGINT', shutdown);
  process.on('SIGTERM', shutdown);
}
