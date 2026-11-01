// A minimal OID4VCI 1.0 wallet for the specs: redeems a Pre-Authorized Code
// offer from the hub's Issuer Service with a `jwt` key proof carrying the
// wallet's P-256 key inline (`jwk`), so the holder is the key's did:jwk.
import { webcrypto } from 'node:crypto';
import { b64url, b64urlJson } from './fake-holder.mjs';

const { subtle } = webcrypto;
const PRE_AUTHORIZED_CODE_GRANT = 'urn:ietf:params:oauth:grant-type:pre-authorized_code';

async function json(res, what) {
  const text = await res.text();
  try {
    return JSON.parse(text);
  } catch {
    throw new Error(`${what} -> ${res.status} ${text}`);
  }
}

/** A fresh wallet key. */
export async function newWallet() {
  const keyPair = await subtle.generateKey({ name: 'ECDSA', namedCurve: 'P-256' }, true, ['sign', 'verify']);
  const { x, y } = await subtle.exportKey('jwk', keyPair.publicKey);
  return { keyPair, jwk: { kty: 'EC', crv: 'P-256', x, y } };
}

async function signProof(wallet, payload) {
  const signingInput = `${b64urlJson({ alg: 'ES256', typ: 'openid4vci-proof+jwt', jwk: wallet.jwk })}.${b64urlJson(payload)}`;
  const sig = await subtle.sign({ name: 'ECDSA', hash: 'SHA-256' }, wallet.keyPair.privateKey, Buffer.from(signingInput));
  return `${signingInput}.${b64url(Buffer.from(sig))}`;
}

/** The operator creates an offer (HTTP Basic with the stack's operator
 * credentials) and `wallet` redeems it for `configurationId`. Returns the
 * Credential Endpoint's `{ status, body }`, whatever it is. */
export async function redeemOffer(stack, wallet, configurationId = 'membership-credential') {
  const base = stack.hub.url;
  const basic = Buffer.from(`${stack.hub.stsClientId}:${stack.hub.stsClientSecret}`).toString('base64');
  const offer = await json(
    await fetch(`${base}/oid4vci/offers`, {
      method: 'POST',
      headers: { authorization: `Basic ${basic}`, 'content-type': 'application/json' },
      body: '{}',
    }),
    'POST /oid4vci/offers',
  );
  const code = offer.credential_offer.grants[PRE_AUTHORIZED_CODE_GRANT]['pre-authorized_code'];
  const token = await json(
    await fetch(`${base}/oid4vci/token`, {
      method: 'POST',
      body: new URLSearchParams({ grant_type: PRE_AUTHORIZED_CODE_GRANT, 'pre-authorized_code': code }),
    }),
    'POST /oid4vci/token',
  );
  const { c_nonce } = await json(await fetch(`${base}/oid4vci/nonce`, { method: 'POST' }), 'POST /oid4vci/nonce');
  const issuer = (await json(await fetch(`${base}/.well-known/openid-credential-issuer`), 'issuer metadata'))
    .credential_issuer;
  const proof = await signProof(wallet, { aud: issuer, iat: Math.floor(Date.now() / 1000), nonce: c_nonce });
  const res = await fetch(`${base}/oid4vci/credential`, {
    method: 'POST',
    headers: { authorization: `Bearer ${token.access_token}`, 'content-type': 'application/json' },
    body: JSON.stringify({ credential_configuration_id: configurationId, proofs: { jwt: [proof] } }),
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
