// Direct backend access for test setup and assertions only (never instead
// of driving the UI): hub admin API, hub public API, registry.
import zlib from 'node:zlib';
import { decodeJws } from './fake-holder.mjs';

const pc = (id) => Buffer.from(id).toString('base64');

async function call(base, token, method, path, body) {
  const res = await fetch(`${base}${path}`, {
    method,
    headers: {
      ...(token ? { authorization: `Bearer ${token}` } : {}),
      ...(body !== undefined ? { 'content-type': 'application/json' } : {}),
    },
    body: body !== undefined ? JSON.stringify(body) : undefined,
  });
  const text = await res.text();
  let json = null;
  try {
    json = text ? JSON.parse(text) : null;
  } catch {
    /* not JSON */
  }
  return { status: res.status, json, text, headers: res.headers };
}

const eq = (field, value) => ({ operandLeft: field, operator: '=', operandRight: value });

export function hubAdmin(stack) {
  const { adminUrl, adminToken, participantContextId } = stack.hub;
  const base = `/v1alpha/participants/${pc(participantContextId)}`;
  const q = async (kind, filters = {}) => {
    const r = await call(adminUrl, adminToken, 'POST', `${base}/${kind}/query`, {
      limit: 500,
      filterExpression: Object.entries(filters).map(([k, v]) => eq(k, v)),
    });
    if (r.status !== 200) throw new Error(`query ${kind} -> ${r.status} ${r.text}`);
    return r.json;
  };
  return {
    raw: (method, path, body) => call(adminUrl, adminToken, method, `${base}${path}`, body),
    definitions: (f) => q('credentialdefinitions', f),
    holders: (f) => q('holders', f),
    processes: (f) => q('issuanceprocesses', f),
    credentials: (f) => q('credentials', f),
    holderByDid: async (did) => (await q('holders', { did }))[0] ?? null,
    createDefinition: (def) => call(adminUrl, adminToken, 'POST', `${base}/credentialdefinitions`, def),
    getDefinition: (id) =>
      call(adminUrl, adminToken, 'GET', `${base}/credentialdefinitions/${encodeURIComponent(id)}`),
  };
}

/** Fetches `GET /statuslist/revocation` (public) and returns the decoded
 * bitstring as a Buffer plus the JWT claims. */
export async function fetchRevocationList(stack) {
  const res = await fetch(`${stack.hub.url}/statuslist/revocation`);
  if (!res.ok) throw new Error(`GET /statuslist/revocation -> ${res.status}`);
  const jwt = (await res.text()).trim();
  const { payload } = decodeJws(jwt);
  const subject = payload.vc?.credentialSubject ?? payload.credentialSubject;
  if (!subject?.encodedList) throw new Error(`status list JWT has no credentialSubject.encodedList`);
  const bits = zlib.gunzipSync(Buffer.from(subject.encodedList, 'base64url'));
  return { bits, payload, contentType: res.headers.get('content-type') };
}

/** Bit `i`, most significant bit first of byte `i / 8` (api-contract.md 1). */
export const bitIsSet = (bits, i) => ((bits[i >> 3] >> (7 - (i & 7))) & 1) === 1;

export function registry(stack) {
  const { url, adminToken } = stack.registry;
  const enc = encodeURIComponent;
  return {
    get: (did) => call(url, null, 'GET', `/v1/issuers/${enc(did)}`),
    put: (rec) => call(url, adminToken, 'PUT', `/v1/issuers/${enc(rec.did)}`, rec),
    setStatus: (did, status) => call(url, adminToken, 'PATCH', `/v1/issuers/${enc(did)}/status`, { status }),
    remove: (did) => call(url, adminToken, 'DELETE', `/v1/issuers/${enc(did)}`),
    check: (did, type) => call(url, null, 'GET', `/v1/issuers/${enc(did)}/types/${enc(type)}`),
  };
}
