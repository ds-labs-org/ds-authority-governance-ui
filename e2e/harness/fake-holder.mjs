// A fake DCP holder (Credential Service side of the Credential Issuance
// Protocol), in plain Node + WebCrypto.
//
// One HTTP server hosts any number of holder identities. Each identity
// `<name>` has:
//   - a fresh P-256 key (WebCrypto, never persisted),
//   - the DID `did:web:127.0.0.1%3A<port>:<name>`, resolved by the hub to
//     `http://127.0.0.1:<port>/<name>/did.json`,
//   - a `CredentialService` endpoint `http://127.0.0.1:<port>/<name>`, so the
//     hub delivers issued credentials with `POST /<name>/credentials`
//     (Storage API) - recorded in `deliveries`.
// The default identity is `holder` (`did:web:127.0.0.1%3A<port>:holder`).
//
// Token shape follows ds-identity-hub-rs `identity-hub-http/src/auth.rs`
// (`verify_bearer_token`): ES256 compact JWS, header `kid` = a
// verificationMethod id that is also listed in `capabilityInvocation`,
// claims iss = sub = holder DID, aud = issuer DID, exp in the future,
// nbf/iat not in the future, unique jti.
//
// Tests run in Playwright worker processes, the server runs in the runner
// process (globalSetup), so everything is also reachable over a small JSON
// control API under `/_control` - see `fakeHolderClient` at the bottom.
//
// Usage standalone: `node harness/fake-holder.mjs [port]`.

import http from 'node:http';
import { randomUUID, webcrypto } from 'node:crypto';
import { pathToFileURL } from 'node:url';

const { subtle } = webcrypto;

export const DCP_CONTEXT = 'https://w3id.org/dspace-dcp/v1.0/dcp.jsonld';
export const DEFAULT_HOLDER_NAME = 'holder';
const NAME_RE = /^[a-z0-9][a-z0-9-]{0,62}$/;

// ---------- JOSE helpers (exported for tests and the unit check) ----------

export const b64url = (data) =>
  Buffer.from(typeof data === 'string' ? Buffer.from(data, 'utf8') : data).toString('base64url');

export const b64urlJson = (value) => b64url(JSON.stringify(value));

export function decodeJws(token) {
  const parts = String(token).split('.');
  if (parts.length !== 3) throw new Error(`not a compact JWS: ${parts.length} segments`);
  return {
    header: JSON.parse(Buffer.from(parts[0], 'base64url').toString('utf8')),
    payload: JSON.parse(Buffer.from(parts[1], 'base64url').toString('utf8')),
    signingInput: `${parts[0]}.${parts[1]}`,
    signature: Buffer.from(parts[2], 'base64url'),
  };
}

/** Signs `payload` as an ES256 compact JWS. WebCrypto ECDSA already yields
 * the raw 64-byte r||s form JWS needs (not DER). */
export async function signEs256(payload, privateKey, kid) {
  const signingInput = `${b64urlJson({ alg: 'ES256', typ: 'JWT', kid })}.${b64urlJson(payload)}`;
  const sig = await subtle.sign(
    { name: 'ECDSA', hash: 'SHA-256' },
    privateKey,
    Buffer.from(signingInput, 'utf8'),
  );
  return `${signingInput}.${b64url(Buffer.from(sig))}`;
}

/** Verifies an ES256 compact JWS against a public JWK. */
export async function verifyEs256(token, publicJwk) {
  const { signingInput, signature } = decodeJws(token);
  const key = await subtle.importKey(
    'jwk',
    { kty: 'EC', crv: 'P-256', x: publicJwk.x, y: publicJwk.y },
    { name: 'ECDSA', namedCurve: 'P-256' },
    false,
    ['verify'],
  );
  return subtle.verify(
    { name: 'ECDSA', hash: 'SHA-256' },
    key,
    signature,
    Buffer.from(signingInput, 'utf8'),
  );
}

/** did:web -> URL, the same simplified rule as dcp_core::did_web_to_url. */
export function didWebToUrl(did, insecureHttp = true) {
  if (!did.startsWith('did:web:')) throw new Error(`not a did:web DID: ${did}`);
  const [host, ...path] = did.slice('did:web:'.length).split(':').map(decodeURIComponent);
  const scheme = insecureHttp ? 'http' : 'https';
  return path.length
    ? `${scheme}://${host}/${path.join('/')}/did.json`
    : `${scheme}://${host}/.well-known/did.json`;
}

const nowSecs = () => Math.floor(Date.now() / 1000);

// ---------- one holder identity ----------

export class HolderIdentity {
  constructor({ name, did, keyId, serviceEndpoint, keyPair, publicJwk }) {
    this.name = name;
    this.did = did;
    this.keyId = keyId;
    this.serviceEndpoint = serviceEndpoint;
    this.keyPair = keyPair;
    this.publicJwk = publicJwk;
    /** Storage API deliveries: { receivedAt, message, authorization, tokenClaims, tokenVerified, tokenError } */
    this.deliveries = [];
    /** Credential requests this holder sent: { id, location, status, holderPid, issuerUrl } */
    this.requests = [];
    /** When set, the Storage API answers with this HTTP status (simulates a broken holder). */
    this.failDeliveriesWith = null;
  }

  static async create({ name, origin, hostPort }) {
    const keyPair = await subtle.generateKey({ name: 'ECDSA', namedCurve: 'P-256' }, true, [
      'sign',
      'verify',
    ]);
    const jwk = await subtle.exportKey('jwk', keyPair.publicKey);
    const did = `did:web:${hostPort.replace(':', '%3A')}:${name}`;
    return new HolderIdentity({
      name,
      did,
      keyId: `${did}#key-1`,
      serviceEndpoint: `${origin}/${name}`,
      keyPair,
      publicJwk: { kty: 'EC', crv: 'P-256', x: jwk.x, y: jwk.y },
    });
  }

  /** The DID document the hub's validator needs: the key in
   * verificationMethod and in authentication / assertionMethod /
   * capabilityInvocation (auth.rs checks capabilityInvocation), plus a
   * CredentialService entry (handlers.rs delivers to `<endpoint>/credentials`). */
  didDocument() {
    return {
      '@context': ['https://www.w3.org/ns/did/v1', 'https://w3id.org/security/suites/jws-2020/v1'],
      id: this.did,
      verificationMethod: [
        {
          id: this.keyId,
          type: 'JsonWebKey2020',
          controller: this.did,
          publicKeyJwk: this.publicJwk,
        },
      ],
      authentication: [this.keyId],
      assertionMethod: [this.keyId],
      capabilityInvocation: [this.keyId],
      service: [
        {
          id: `${this.did}#credential-service`,
          type: 'CredentialService',
          serviceEndpoint: this.serviceEndpoint,
        },
      ],
    };
  }

  /** A Self-Issued ID Token addressed to `aud`. `overrides` replaces or
   * (with value `undefined`) removes claims - handy for negative tests. */
  async selfIssuedToken(aud, overrides = {}) {
    const now = nowSecs();
    const claims = {
      iss: this.did,
      sub: this.did,
      aud,
      iat: now,
      nbf: now,
      exp: now + 300,
      jti: randomUUID(),
      ...overrides,
    };
    for (const [k, v] of Object.entries(claims)) if (v === undefined) delete claims[k];
    return signEs256(claims, this.keyPair.privateKey, this.keyId);
  }

  /** Sends a DCP CredentialRequestMessage to the issuer's public
   * `POST /credentials`. Resolves the issuer DID from `GET /metadata` when
   * not given. Never throws on an HTTP error status: returns it. */
  async requestCredential({
    issuerUrl,
    issuerDid,
    credentials = ['membership-credential'],
    holderPid = `hpid-${randomUUID()}`,
  }) {
    const base = issuerUrl.replace(/\/$/, '');
    const aud = issuerDid ?? (await fetchIssuerDid(base));
    const token = await this.selfIssuedToken(aud);
    const res = await fetch(`${base}/credentials`, {
      method: 'POST',
      headers: { 'content-type': 'application/json', authorization: `Bearer ${token}` },
      body: JSON.stringify({
        '@context': [DCP_CONTEXT],
        type: 'CredentialRequestMessage',
        holderPid,
        credentials: credentials.map((id) => ({ id })),
      }),
    });
    const body = await res.text();
    const location = res.headers.get('location');
    const id = location ? location.split('/').filter(Boolean).pop() : null;
    const result = { status: res.status, location, id, holderPid, body, issuerUrl: base };
    this.requests.push(result);
    return result;
  }

  /** `GET /requests/{id}` on the issuer (DCP Credential Request Status API).
   * The hub does not authenticate it today; a token is sent anyway, as DCP
   * expects one. */
  async requestStatus({ issuerUrl, issuerDid, id }) {
    const base = issuerUrl.replace(/\/$/, '');
    const aud = issuerDid ?? (await fetchIssuerDid(base));
    const token = await this.selfIssuedToken(aud);
    const res = await fetch(`${base}/requests/${encodeURIComponent(id)}`, {
      headers: { authorization: `Bearer ${token}` },
    });
    const text = await res.text();
    let body = text;
    try {
      body = JSON.parse(text);
    } catch {
      /* keep text */
    }
    return { status: res.status, body };
  }

  /** Storage API: records a CredentialMessage delivery. The issuer's own
   * Self-Issued ID Token is decoded and, when its DID resolves, verified. */
  async receiveDelivery(message, authorization) {
    const record = {
      receivedAt: nowSecs(),
      message,
      authorization: authorization ?? null,
      tokenClaims: null,
      tokenVerified: false,
      tokenError: null,
    };
    try {
      const token = (authorization ?? '').replace(/^Bearer /, '');
      const { header, payload } = decodeJws(token);
      record.tokenClaims = payload;
      if (payload.aud !== this.did) throw new Error(`aud ${payload.aud} != ${this.did}`);
      const doc = await (await fetch(didWebToUrl(payload.iss))).json();
      const vm = (doc.verificationMethod ?? []).find((m) => m.id === header.kid);
      if (!vm) throw new Error(`kid ${header.kid} not in issuer DID document`);
      record.tokenVerified = await verifyEs256(token, vm.publicKeyJwk);
    } catch (err) {
      record.tokenError = String(err?.message ?? err);
    }
    this.deliveries.push(record);
    return record;
  }

  info() {
    return {
      name: this.name,
      did: this.did,
      keyId: this.keyId,
      serviceEndpoint: this.serviceEndpoint,
      failDeliveriesWith: this.failDeliveriesWith,
      requests: this.requests,
      deliveries: this.deliveries,
    };
  }
}

export async function fetchIssuerDid(issuerUrl) {
  const res = await fetch(`${issuerUrl.replace(/\/$/, '')}/metadata`);
  if (!res.ok) throw new Error(`GET ${issuerUrl}/metadata -> ${res.status}`);
  const meta = await res.json();
  if (!meta.issuer) throw new Error('issuer metadata has no `issuer`');
  return meta.issuer;
}

// ---------- the HTTP server ----------

async function readJson(req) {
  const chunks = [];
  for await (const c of req) chunks.push(c);
  const raw = Buffer.concat(chunks).toString('utf8');
  return raw ? JSON.parse(raw) : {};
}

function send(res, status, body) {
  if (body === undefined) {
    res.writeHead(status).end();
    return;
  }
  res.writeHead(status, { 'content-type': 'application/json' }).end(JSON.stringify(body));
}

/**
 * Starts the fake holder server. Returns
 * `{ origin, port, controlUrl, holders, createHolder(name?), holder(name), close() }`.
 */
export async function startFakeHolder({ host = '127.0.0.1', port = 0, log = () => {} } = {}) {
  const holders = new Map();
  let origin;
  let hostPort;

  const createHolder = async (name = `h-${randomUUID().slice(0, 8)}`) => {
    if (!NAME_RE.test(name)) throw new Error(`invalid holder name '${name}'`);
    if (holders.has(name)) throw new Error(`holder '${name}' exists`);
    const h = await HolderIdentity.create({ name, origin, hostPort });
    holders.set(name, h);
    log(`created holder ${h.did}`);
    return h;
  };

  const server = http.createServer(async (req, res) => {
    try {
      const url = new URL(req.url, 'http://x');
      const seg = url.pathname.split('/').filter(Boolean).map(decodeURIComponent);

      // --- identity routes ---
      if (req.method === 'GET' && seg.length === 2 && seg[1] === 'did.json') {
        const h = holders.get(seg[0]);
        return h ? send(res, 200, h.didDocument()) : send(res, 404, { error: 'unknown holder' });
      }
      if (req.method === 'POST' && seg.length === 2 && seg[1] === 'credentials') {
        const h = holders.get(seg[0]);
        if (!h) return send(res, 404, { error: 'unknown holder' });
        const message = await readJson(req);
        const record = await h.receiveDelivery(message, req.headers.authorization);
        log(`delivery to ${h.name}: issuerPid=${message.issuerPid} verified=${record.tokenVerified}`);
        if (h.failDeliveriesWith) return send(res, h.failDeliveriesWith, { error: 'configured to fail' });
        return send(res, 200, {});
      }

      // --- control API ---
      if (seg[0] === '_control') {
        if (req.method === 'GET' && seg.length === 1) {
          return send(res, 200, { origin, holders: [...holders.keys()] });
        }
        if (req.method === 'POST' && seg.length === 2 && seg[1] === 'holders') {
          const { name } = await readJson(req);
          const h = await createHolder(name);
          return send(res, 201, h.info());
        }
        if (seg[1] === 'holders' && seg.length >= 3) {
          const h = holders.get(seg[2]);
          if (!h) return send(res, 404, { error: `unknown holder ${seg[2]}` });
          const rest = seg.slice(3);
          if (req.method === 'GET' && rest.length === 0) return send(res, 200, h.info());
          if (req.method === 'PATCH' && rest.length === 0) {
            const body = await readJson(req);
            if ('failDeliveriesWith' in body) h.failDeliveriesWith = body.failDeliveriesWith;
            return send(res, 200, h.info());
          }
          if (req.method === 'GET' && rest[0] === 'deliveries') return send(res, 200, h.deliveries);
          if (req.method === 'POST' && rest[0] === 'token') {
            const { aud, overrides } = await readJson(req);
            return send(res, 200, { token: await h.selfIssuedToken(aud, overrides) });
          }
          if (req.method === 'POST' && rest[0] === 'credential-requests' && rest.length === 1) {
            return send(res, 200, await h.requestCredential(await readJson(req)));
          }
          if (req.method === 'GET' && rest[0] === 'credential-requests' && rest.length === 2) {
            const issuerUrl = url.searchParams.get('issuerUrl');
            if (!issuerUrl) return send(res, 400, { error: 'issuerUrl query parameter required' });
            return send(res, 200, await h.requestStatus({ issuerUrl, id: rest[1] }));
          }
        }
      }
      send(res, 404, { error: `no route ${req.method} ${url.pathname}` });
    } catch (err) {
      send(res, 500, { error: String(err?.stack ?? err) });
    }
  });

  await new Promise((resolve, reject) => {
    server.once('error', reject);
    server.listen(port, host, resolve);
  });
  const actualPort = server.address().port;
  hostPort = `${host}:${actualPort}`;
  origin = `http://${hostPort}`;
  await createHolder(DEFAULT_HOLDER_NAME);

  return {
    origin,
    port: actualPort,
    controlUrl: `${origin}/_control`,
    holders,
    createHolder,
    holder: (name = DEFAULT_HOLDER_NAME) => holders.get(name),
    close: () =>
      new Promise((resolve) => {
        server.closeAllConnections?.();
        server.close(() => resolve());
      }),
  };
}

// ---------- client for the control API (used from Playwright workers) ----------

export function fakeHolderClient(origin) {
  const base = `${origin.replace(/\/$/, '')}/_control`;
  const call = async (method, path, body) => {
    const res = await fetch(`${base}${path}`, {
      method,
      headers: body ? { 'content-type': 'application/json' } : {},
      body: body ? JSON.stringify(body) : undefined,
    });
    const json = await res.json();
    if (!res.ok) throw new Error(`fake holder ${method} ${path} -> ${res.status}: ${json.error}`);
    return json;
  };
  return {
    /** Creates a fresh holder identity (fresh DID + key). */
    create: (name) => call('POST', '/holders', name ? { name } : {}),
    get: (name) => call('GET', `/holders/${name}`),
    setFailDeliveries: (name, status) =>
      call('PATCH', `/holders/${name}`, { failDeliveriesWith: status }),
    deliveries: (name) => call('GET', `/holders/${name}/deliveries`),
    token: (name, aud, overrides) => call('POST', `/holders/${name}/token`, { aud, overrides }),
    /** -> { status, location, id, holderPid, body } (HTTP errors are returned, not thrown). */
    requestCredential: (name, args) => call('POST', `/holders/${name}/credential-requests`, args),
    requestStatus: (name, issuerUrl, id) =>
      call(
        'GET',
        `/holders/${name}/credential-requests/${encodeURIComponent(id)}?issuerUrl=${encodeURIComponent(issuerUrl)}`,
      ),
  };
}

if (import.meta.url === pathToFileURL(process.argv[1] ?? '').href) {
  const server = await startFakeHolder({ port: Number(process.argv[2] ?? 0), log: console.log });
  console.log(`fake holder on ${server.origin}, default DID ${server.holder().did}`);
}
