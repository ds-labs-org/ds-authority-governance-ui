// Non-regression checks for the 2026-10-16 project audit (findings C1, E1,
// E2, E3) and the 2026-11-01 one (E5, M1, M2, M3, N1, N2), against the real
// hub and registry binaries and, where the finding is visible there, through
// the console.
// E4 (the harness could not start the registry) has no spec of its own: the
// whole suite is its check.
import { execFileSync, spawn } from 'node:child_process';
import fs from 'node:fs';
import http from 'node:http';
import https from 'node:https';
import os from 'node:os';
import path from 'node:path';
import {
  test, expect, uid, gotoRoute, pageHeading, expectLoaded, table, row, rowFor, dialog, errorAlert, refreshUntil,
} from './fixtures.mjs';
import { fetchRevocationList, bitIsSet } from '../harness/backend.mjs';
import { HolderIdentity, decodeJws, signEs256, verifyEs256 } from '../harness/fake-holder.mjs';
import { freePort } from '../harness/stack.mjs';
import { newWallet, redeemOffer } from '../harness/wallet.mjs';

const nowSecs = () => Math.floor(Date.now() / 1000);
const MEMBERSHIP_SCOPE = 'org.eclipse.dspace.dcp.vc.type:MembershipCredential';

/** Runs `bin args` to completion; resolves `{ code, stderr }`. */
function runToExit(bin, args, env) {
  return new Promise((resolve, reject) => {
    const child = spawn(bin, args, { env, stdio: ['ignore', 'ignore', 'pipe'] });
    let stderr = '';
    child.stderr.on('data', (d) => (stderr += d));
    const timer = setTimeout(() => {
      child.kill('SIGKILL');
      reject(new Error(`${bin} ${args.join(' ')} was still running after 10 s: it started`));
    }, 10_000);
    child.on('exit', (code) => {
      clearTimeout(timer);
      resolve({ code, stderr });
    });
  });
}

/** The process environment without the hub's secret variables. */
function envWithoutHubSecrets() {
  const env = { ...process.env };
  delete env.IDENTITY_HUB_STS_CLIENT_SECRET;
  delete env.IDENTITY_HUB_ADMIN_TOKEN;
  return env;
}

test.describe('C1: the hub has no default operator secret', () => {
  test('the hub binary refuses to start without --sts-client-secret', async ({ stack }) => {
    test.skip(!stack.hub.bin, 'the hub was not started by the harness (HUB_URL)');
    const port = await freePort();
    for (const mode of ['issuer-service', 'credential-service']) {
      const { code, stderr } = await runToExit(
        stack.hub.bin,
        [mode, '--bind', `127.0.0.1:${port}`],
        envWithoutHubSecrets(),
      );
      expect(code, `${mode} exited`).not.toBe(0);
      expect(stderr).toContain('--sts-client-secret');
    }
  });

  test("the DCP TCK's client credentials open neither the STS nor OID4VCI offers", async ({ stack }) => {
    const sts = await fetch(`${stack.hub.url}/sts/token`, {
      method: 'POST',
      body: new URLSearchParams({
        grant_type: 'client_credentials',
        client_id: 'tck-client',
        client_secret: 'tck-secret',
        audience: stack.hub.did,
      }),
    });
    expect(sts.status).toBeGreaterThanOrEqual(400);
    expect(sts.status).toBeLessThan(500);

    const offer = (id, secret) =>
      fetch(`${stack.hub.url}/oid4vci/offers`, {
        method: 'POST',
        headers: {
          authorization: `Basic ${Buffer.from(`${id}:${secret}`).toString('base64')}`,
          'content-type': 'application/json',
        },
        body: '{}',
      });
    expect((await offer('tck-client', 'tck-secret')).status).toBe(401);
    expect((await offer(stack.hub.stsClientId, stack.hub.stsClientSecret)).status).toBe(201);
  });
});

test.describe('E1: OID4VCI issuance is governed in the console', () => {
  test('a redeemed offer shows up in the console and is revoked there', async ({ page, stack, hub }) => {
    const wallet = await newWallet();
    const redeemed = await redeemOffer(stack, wallet);
    expect(redeemed.status, JSON.stringify(redeemed.body)).toBe(200);
    const { payload: vc } = decodeJws(redeemed.body.credentials[0].credential);
    const holderDid = vc.sub;
    expect(holderDid).toMatch(/^did:jwk:/);
    expect(vc.vc.credentialStatus?.type).toBe('StatusList2021Entry');
    const index = Number(vc.vc.credentialStatus.statusListIndex);

    // Holders: the wallet's DID is listed, pending (the offer authorized this
    // credential, not the DID's future DCP requests).
    await gotoRoute(page, '/holders');
    await expect(pageHeading(page, 'Holders')).toBeVisible();
    await expectLoaded(page);
    await expect(row(table(page, 'All holders'), holderDid)).toContainText(/pending/i);

    // Credentials: a DELIVERED process and an active credential.
    const [record] = await hub.credentials({ holderDid });
    expect(record).toMatchObject({ id: vc.vc.id, status: 'active', statusListIndex: index });
    await gotoRoute(page, '/credentials');
    await expect(pageHeading(page, 'Credentials')).toBeVisible();
    await expectLoaded(page);
    await expect(row(table(page, 'Issuance processes'), record.issuanceProcessId)).toContainText('DELIVERED');
    const issued = table(page, 'Issued credentials');
    await expect(row(issued, holderDid)).toContainText('MembershipCredential');
    await expect(row(issued, holderDid)).toContainText(/active/i);

    // Revoke through the UI: the public status list has the bit.
    expect(bitIsSet((await fetchRevocationList(stack)).bits, index)).toBe(false);
    await row(issued, holderDid).getByRole('button', { name: 'Revoke', exact: true }).click();
    const dlg = dialog(page, 'Revoke credential');
    await dlg.getByRole('button', { name: 'Revoke', exact: true }).click();
    await expect(dlg).toBeHidden();
    await expect(row(table(page, 'Issued credentials'), holderDid)).toContainText(/revoked/i);
    expect(bitIsSet((await fetchRevocationList(stack)).bits, index)).toBe(true);
  });

  test('a holder rejected in the console cannot redeem another offer', async ({ page, stack, hub }) => {
    const wallet = await newWallet();
    const first = await redeemOffer(stack, wallet);
    expect(first.status, JSON.stringify(first.body)).toBe(200);
    const holderDid = decodeJws(first.body.credentials[0].credential).payload.sub;

    await gotoRoute(page, '/holders');
    await expectLoaded(page);
    await row(table(page, 'Pending approval'), holderDid).getByRole('button', { name: 'Reject', exact: true }).click();
    const dlg = dialog(page, 'Reject holder');
    await dlg.getByRole('button', { name: 'Reject', exact: true }).click();
    await expect(dlg).toBeHidden();
    await expect(row(table(page, 'All holders'), holderDid)).toContainText(/rejected/i);

    const again = await redeemOffer(stack, wallet);
    expect(again.status).toBe(400);
    expect(again.body.error).toBe('credential_request_denied');
    expect(await hub.credentials({ holderDid })).toHaveLength(1);
  });
});

/** A did:web party server on 127.0.0.1 hosting `names` (HolderIdentity). */
async function startParties(names) {
  const port = await freePort();
  const origin = `http://127.0.0.1:${port}`;
  const parties = {};
  for (const name of names) {
    parties[name] = await HolderIdentity.create({ name, origin, hostPort: `127.0.0.1:${port}` });
  }
  const server = http.createServer((req, res) => {
    const name = req.url.split('/')[1];
    if (parties[name] && req.url === `/${name}/did.json`) {
      res.writeHead(200, { 'content-type': 'application/json' });
      return res.end(JSON.stringify(parties[name].didDocument()));
    }
    res.writeHead(404);
    res.end();
  });
  await new Promise((resolve) => server.listen(port, '127.0.0.1', resolve));
  return { parties, close: () => new Promise((resolve) => server.close(() => resolve())) };
}

/** A Credential Service hub (the real binary) trusting `trustedDid`. */
async function startCredentialService(bin, trustedDid, extraArgs = []) {
  const port = await freePort();
  const secret = `cs-${uid('secret')}`;
  const child = spawn(
    bin,
    [
      'credential-service',
      '--bind', `127.0.0.1:${port}`,
      '--did-host', `127.0.0.1:${port}`,
      '--sts-client-secret', secret,
      '--trusted-issuer-did', trustedDid,
      ...extraArgs,
    ],
    { env: envWithoutHubSecrets(), stdio: 'ignore' },
  );
  const url = `http://127.0.0.1:${port}`;
  await expect(async () => {
    expect((await fetch(`${url}/credential-service/did.json`)).ok).toBe(true);
  }).toPass({ timeout: 15_000 });
  return {
    url,
    did: `did:web:127.0.0.1%3A${port}:credential-service`,
    secret,
    stop: () => child.kill('SIGTERM'),
  };
}

/** A MembershipCredential JWT signed by `issuer` for `subject`, expiring
 * `expFromNow` seconds from now. */
function membershipVc(issuer, subject, expFromNow) {
  const now = nowSecs();
  return signEs256(
    {
      iss: issuer.did, sub: subject, iat: now - 60, nbf: now - 60, exp: now + expFromNow,
      jti: `urn:uuid:${uid('vc')}`,
      vc: {
        '@context': ['https://www.w3.org/2018/credentials/v1'],
        type: ['VerifiableCredential', 'MembershipCredential'],
        issuer: issuer.did,
        credentialSubject: { id: subject },
      },
    },
    issuer.keyPair.privateKey,
    issuer.keyId,
  );
}

/** `deliverer` sends `payload` to the Credential Service `cs`'s Storage API,
 * filed as `credentialType`; resolves the HTTP status. */
async function deliverTo(cs, deliverer, payload, credentialType = 'MembershipCredential') {
  const token = await deliverer.selfIssuedToken(cs.did);
  const res = await fetch(`${cs.url}/credentials`, {
    method: 'POST',
    headers: { authorization: `Bearer ${token}`, 'content-type': 'application/json' },
    body: JSON.stringify({
      '@context': ['https://w3id.org/dspace-dcp/v1.0/dcp.jsonld'],
      type: 'CredentialMessage',
      issuerPid: 'issuer-pid', holderPid: 'holder-pid', status: 'ISSUED',
      credentials: [{ credentialType, payload, format: 'jwt' }],
    }),
  });
  return res.status;
}

test.describe('E2: a Credential Service holds and presents only valid credentials of its own', () => {
  test('the Storage API refuses foreign, expired and untrusted credentials; presentations drop expired and repeats', async ({
    stack,
  }) => {
    test.skip(!stack.hub.bin, 'needs the hub binary (HUB_URL is set)');
    const { parties, close } = await startParties(['issuer', 'rogue']);
    const cs = await startCredentialService(stack.hub.bin, parties.issuer.did);
    try {
      const vc = membershipVc;
      const deliver = (payload) => deliverTo(cs, parties.issuer, payload);

      expect(await deliver(await vc(parties.rogue, cs.did, 3600)), 'signed by an untrusted issuer').toBe(400);
      expect(await deliver(await vc(parties.issuer, cs.did, -3600)), 'already expired').toBe(400);
      expect(await deliver(await vc(parties.issuer, 'did:web:someone-else.example', 3600)), 'not ours').toBe(400);

      const valid = await vc(parties.issuer, cs.did, 3600);
      const shortLived = await vc(parties.issuer, cs.did, 2);
      expect(await deliver(valid)).toBe(200);
      expect(await deliver(valid), 'the same credential again').toBe(200);
      expect(await deliver(shortLived)).toBe(200);
      await new Promise((r) => setTimeout(r, 3000)); // shortLived expires

      const sts = await (
        await fetch(`${cs.url}/sts/token`, {
          method: 'POST',
          body: new URLSearchParams({
            grant_type: 'client_credentials',
            client_id: 'tck-client',
            client_secret: cs.secret,
            audience: cs.did,
            bearer_access_scope: MEMBERSHIP_SCOPE,
          }),
        })
      ).json();
      const res = await fetch(`${cs.url}/presentations/query`, {
        method: 'POST',
        headers: { authorization: `Bearer ${sts.access_token}`, 'content-type': 'application/json' },
        body: JSON.stringify({
          '@context': ['https://w3id.org/dspace-dcp/v1.0/dcp.jsonld'],
          type: 'PresentationQueryMessage',
          scope: [MEMBERSHIP_SCOPE],
        }),
      });
      expect(res.status).toBe(200);
      const vp = decodeJws((await res.json()).presentation[0]).payload;
      expect(vp.vp.verifiableCredential, 'the valid credential, once').toEqual([valid]);
    } finally {
      cs.stop();
      await close();
    }
  });
});

test.describe('E3: narrowing a TAO in the console keeps the registry usable', () => {
  test('the registry restarts from its data file and its snapshot verifies', async ({ page, stack, tir }) => {
    const root = `did:web:${uid('root')}.example`;
    const tao = `did:web:${uid('tao')}.example`;
    const ti = `did:web:${uid('ti')}.example`;
    for (const rec of [
      { did: root, role: 'root_tao', credentialTypes: ['*'], status: 'active' },
      { did: tao, role: 'tao', credentialTypes: ['MembershipCredential', 'DataProcessorCredential'], accreditedBy: root, status: 'active' },
      { did: ti, role: 'ti', credentialTypes: ['MembershipCredential'], accreditedBy: tao, status: 'active' },
    ]) {
      expect((await tir.put(rec)).status).toBe(201);
    }

    // Narrow the TAO through the console.
    await gotoRoute(page, '/trusted-issuers');
    await expect(pageHeading(page, 'Trusted Issuers')).toBeVisible();
    await expectLoaded(page);
    await rowFor(table(page, 'Trusted issuers'), tao).getByRole('button', { name: 'Edit', exact: true }).click();
    const dlg = dialog(page, 'Edit trusted issuer');
    await dlg.getByLabel('Credential types', { exact: true }).fill('DataProcessorCredential');
    await dlg.getByRole('button', { name: 'Save', exact: true }).click();
    await expect(dlg).toBeHidden();
    await expect(errorAlert(page, 'Could not save trusted issuer')).toHaveCount(0);
    expect((await tir.get(tao)).json.credentialTypes).toEqual(['DataProcessorCredential']);

    // The registry comes back from its own data file.
    const restart = await fetch(`${stack.harnessUrl}/registry/restart`, { method: 'POST' });
    test.skip(restart.status === 501, 'the registry was not started by the harness (REGISTRY_URL)');
    expect(restart.status, await restart.text()).toBe(200);

    // The console still lists the chain; the TI lost only MembershipCredential.
    await page.reload();
    await expectLoaded(page);
    await expect(rowFor(table(page, 'Trusted issuers'), ti)).toBeVisible();
    const form = page.getByRole('form', { name: 'Check trust' });
    await form.getByLabel('Issuer DID', { exact: true }).fill(ti);
    await form.getByLabel('Credential type', { exact: true }).fill('MembershipCredential');
    await form.getByRole('button', { name: 'Check', exact: true }).click();
    await expect(form.getByRole('status')).toContainText(`'${tao}' is not scoped`);
    await expect(form.getByRole('status')).toContainText('Not trusted');

    // Its signed snapshot verifies against its DID document and lists the TI.
    const snapshot = (await (await fetch(`${stack.registry.url}/v1/snapshot`)).text()).trim();
    const { header, payload } = decodeJws(snapshot);
    const doc = await (await fetch(`${stack.registry.url}/.well-known/did.json`)).json();
    const key = doc.verificationMethod.find((m) => m.id === header.kid);
    expect(key, `kid ${header.kid} in the registry DID document`).toBeTruthy();
    expect(await verifyEs256(snapshot, key.publicKeyJwk)).toBe(true);
    expect(JSON.stringify(payload)).toContain(ti);

    await tir.remove(ti);
    await tir.remove(tao);
    await tir.remove(root);
  });
});

// ---------- 2026-11-01 audit ----------

/** Runs openssl in `dir`. */
function openssl(dir, args) {
  execFileSync('openssl', args, { cwd: dir, stdio: ['ignore', 'ignore', 'pipe'] });
}

/** A P-256 certificate `name` in `dir` (`name.key`, `name.pem`), signed by
 * `parent` (self-signed when null), with the X.509 v3 extensions `ext`. */
function certificate(dir, name, parent, ext) {
  fs.writeFileSync(path.join(dir, `${name}.ext`), `${ext.join('\n')}\n`);
  openssl(dir, ['ecparam', '-name', 'prime256v1', '-genkey', '-noout', '-out', `${name}.key`]);
  openssl(dir, ['req', '-new', '-key', `${name}.key`, '-subj', `/CN=${name}`, '-out', `${name}.csr`]);
  const signer = parent
    ? ['-CA', `${parent}.pem`, '-CAkey', `${parent}.key`, '-CAcreateserial']
    : ['-signkey', `${name}.key`];
  openssl(dir, [
    'x509', '-req', '-in', `${name}.csr`, ...signer, '-days', '2', '-sha256',
    '-extfile', `${name}.ext`, '-out', `${name}.pem`,
  ]);
  return {
    pem: path.join(dir, `${name}.pem`),
    key: path.join(dir, `${name}.key`),
    /** base64 DER, as in `x5c` and the registry's `x509Certificates`. */
    der: fs.readFileSync(path.join(dir, `${name}.pem`), 'utf8').replace(/-----[^-]+-----|\s/g, ''),
  };
}

const CA_EXT = ['basicConstraints=critical,CA:TRUE', 'keyUsage=critical,keyCertSign,cRLSign', 'subjectKeyIdentifier=hash'];

const credentialRequest = () =>
  JSON.stringify({
    '@context': ['https://w3id.org/dspace-dcp/v1.0/dcp.jsonld'],
    type: 'CredentialRequestMessage',
    holderPid: uid('hpid'),
    credentials: [{ id: 'membership-credential' }],
  });

/** Listens on `host:port` (port 0: any); resolves the bound port. */
const listen = (server, port, host) =>
  new Promise((resolve) => server.listen(port, host, () => resolve(server.address().port)));
const closeServer = (server) =>
  new Promise((resolve) => {
    server.closeAllConnections?.();
    server.close(() => resolve());
  });

test.describe('M1: a redirect cannot take the hub off its outbound allow-list', () => {
  test('a DID document that redirects to an unconfigured host is not followed', async ({ stack }) => {
    test.skip(!stack.hub.bin, 'the probe must be on the hub host (HUB_URL is set)');
    let hits = 0;
    const probe = http.createServer((req, res) => {
      hits += 1;
      res.writeHead(200, { 'content-type': 'application/json' });
      res.end('{}');
    });
    const probePort = await listen(probe, 0, '127.0.0.2');
    const port = await freePort();
    const attacker = await HolderIdentity.create({
      name: 'attacker', origin: `http://127.0.0.1:${port}`, hostPort: `127.0.0.1:${port}`,
    });
    const redirector = http.createServer((req, res) => {
      res.writeHead(302, { location: `http://127.0.0.2:${probePort}${req.url}` });
      res.end();
    });
    await listen(redirector, port, '127.0.0.1');
    try {
      const res = await fetch(`${stack.hub.url}/credentials`, {
        method: 'POST',
        headers: {
          authorization: `Bearer ${await attacker.selfIssuedToken(stack.hub.did)}`,
          'content-type': 'application/json',
        },
        body: credentialRequest(),
      });
      expect(res.status).toBeGreaterThanOrEqual(400);
      expect(res.status).toBeLessThan(500);
      expect(hits, 'requests the hub sent to 127.0.0.2, which no part of its configuration names').toBe(0);
    } finally {
      await closeServer(redirector);
      await closeServer(probe);
    }
  });
});

test.describe('M2: a holder rejected in the console stays rejected', () => {
  test('respelling its DID (%3a for %3A) gets neither a credential nor a second record', async ({
    page, holders, hub, stack,
  }) => {
    const h = await holders.create(uid('holder'));
    expect((await holders.requestCredential(h.name, { issuerUrl: stack.hub.url })).status).toBe(201);
    await gotoRoute(page, '/holders');
    await expectLoaded(page);
    await row(table(page, 'Pending approval'), h.did).getByRole('button', { name: 'Reject', exact: true }).click();
    const dlg = dialog(page, 'Reject holder');
    await dlg.getByRole('button', { name: 'Reject', exact: true }).click();
    await expect(dlg).toBeHidden();
    await expect(row(table(page, 'All holders'), h.did)).toContainText(/rejected/i);

    const respelled = h.did.replace('%3A', '%3a');
    expect(respelled).not.toBe(h.did);
    const { token } = await holders.token(h.name, stack.hub.did, { iss: respelled, sub: respelled });
    const res = await fetch(`${stack.hub.url}/credentials`, {
      method: 'POST',
      headers: { authorization: `Bearer ${token}`, 'content-type': 'application/json' },
      body: credentialRequest(),
    });
    expect(res.status, await res.text()).toBe(403);
    const same = (await hub.holders()).filter((x) => x.did.toLowerCase() === h.did.toLowerCase());
    expect(same, 'one holder record for the party').toHaveLength(1);
    expect(await holders.deliveries(h.name)).toHaveLength(0);
  });
});

test.describe('M3: issued credentials carry their issuance date', () => {
  test('a credential approved in the console, and one redeemed over OID4VCI, have nbf and issuanceDate', async ({
    page, holders, stack,
  }) => {
    const xsd = (secs) => new Date(secs * 1000).toISOString().replace('.000Z', 'Z');
    const expectDates = (jws, where) => {
      const { payload } = decodeJws(jws);
      expect(payload.nbf, `${where}: nbf`).toBe(payload.iat);
      expect(payload.vc.issuanceDate, `${where}: issuanceDate`).toBe(xsd(payload.iat));
      expect(payload.vc.expirationDate, `${where}: expirationDate`).toBe(xsd(payload.exp));
    };

    const h = await holders.create(uid('holder'));
    expect((await holders.requestCredential(h.name, { issuerUrl: stack.hub.url })).status).toBe(201);
    await gotoRoute(page, '/holders');
    await expectLoaded(page);
    await row(table(page, 'Pending approval'), h.did).getByRole('button', { name: 'Approve', exact: true }).click();
    await expect(row(table(page, 'All holders'), h.did)).toContainText(/approved/i);
    let deliveries = [];
    await expect(async () => {
      deliveries = await holders.deliveries(h.name);
      expect(deliveries).toHaveLength(1);
    }).toPass({ timeout: 10_000 });
    expectDates(deliveries[0].message.credentials[0].payload, 'DCP');

    const redeemed = await redeemOffer(stack, await newWallet());
    expect(redeemed.status, JSON.stringify(redeemed.body)).toBe(200);
    expectDates(redeemed.body.credentials[0].credential, 'OID4VCI');
  });
});

test.describe('N2: the Storage API files a credential only under its own type', () => {
  test('a MembershipCredential labelled DataProcessorCredential is refused', async ({ stack }) => {
    test.skip(!stack.hub.bin, 'needs the hub binary (HUB_URL is set)');
    const { parties, close } = await startParties(['issuer']);
    const cs = await startCredentialService(stack.hub.bin, parties.issuer.did);
    try {
      const vc = await membershipVc(parties.issuer, cs.did, 3600);
      expect(await deliverTo(cs, parties.issuer, vc, 'DataProcessorCredential'), 'filed under another type').toBe(400);
      expect(await deliverTo(cs, parties.issuer, vc, 'MembershipCredential'), 'filed under its own type').toBe(200);
    } finally {
      cs.stop();
      await close();
    }
  });
});

test.describe('E5: presentations work with --insecure-http false', () => {
  test("a verifier forwarding the hub's own STS token gets the credential, every DID over https", async ({
    stack,
  }) => {
    test.skip(!stack.hub.bin, 'needs the hub binary (HUB_URL is set)');
    const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'audit-e5-'));
    const root = certificate(dir, 'tls-root', null, CA_EXT);
    const leaf = certificate(dir, 'tls-leaf', 'tls-root', [
      'basicConstraints=critical,CA:FALSE', 'keyUsage=critical,digitalSignature',
      'extendedKeyUsage=serverAuth', 'subjectAltName=IP:127.0.0.1',
    ]);

    // The issuer and the verifier publish their DID documents over https only.
    const port = await freePort();
    const parties = {};
    for (const name of ['issuer', 'verifier']) {
      parties[name] = await HolderIdentity.create({
        name, origin: `https://127.0.0.1:${port}`, hostPort: `127.0.0.1:${port}`,
      });
    }
    const server = https.createServer(
      { key: fs.readFileSync(leaf.key), cert: fs.readFileSync(leaf.pem) },
      (req, res) => {
        const party = parties[req.url.split('/')[1]];
        if (party && req.url === `/${party.name}/did.json`) {
          res.writeHead(200, { 'content-type': 'application/json' });
          return res.end(JSON.stringify(party.didDocument()));
        }
        res.writeHead(404);
        res.end();
      },
    );
    await listen(server, port, '127.0.0.1');
    const cs = await startCredentialService(stack.hub.bin, parties.issuer.did, [
      '--insecure-http', 'false', '--extra-root-cert', root.pem,
    ]);
    try {
      const vc = await membershipVc(parties.issuer, cs.did, 3600);
      expect(await deliverTo(cs, parties.issuer, vc), 'stored, the issuer resolved over https').toBe(200);

      const sts = await (
        await fetch(`${cs.url}/sts/token`, {
          method: 'POST',
          body: new URLSearchParams({
            grant_type: 'client_credentials',
            client_id: 'tck-client',
            client_secret: cs.secret,
            audience: parties.verifier.did,
            bearer_access_scope: MEMBERSHIP_SCOPE,
          }),
        })
      ).json();
      const nested = decodeJws(sts.access_token).payload.token;
      const outer = await parties.verifier.selfIssuedToken(cs.did, { token: nested });
      const res = await fetch(`${cs.url}/presentations/query`, {
        method: 'POST',
        headers: { authorization: `Bearer ${outer}`, 'content-type': 'application/json' },
        body: JSON.stringify({
          '@context': ['https://w3id.org/dspace-dcp/v1.0/dcp.jsonld'],
          type: 'PresentationQueryMessage',
          scope: [MEMBERSHIP_SCOPE],
        }),
      });
      const body = await res.text();
      expect(res.status, body).toBe(200);
      const vp = decodeJws(JSON.parse(body).presentation[0]).payload;
      expect(vp.vp.verifiableCredential).toEqual([vc]);
    } finally {
      cs.stop();
      await closeServer(server);
      fs.rmSync(dir, { recursive: true, force: true });
    }
  });
});

test.describe('N1: an X.509 chain is judged by the party whose anchor is nearest the leaf', () => {
  test("an issuer suspended in the console stops being trusted through its TAO's root anchor", async ({
    page, tir, stack,
  }) => {
    const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'audit-n1-'));
    try {
      const rootCa = certificate(dir, 'root-ca', null, CA_EXT);
      const issuingCa = certificate(dir, 'issuing-ca', 'root-ca', [...CA_EXT, 'authorityKeyIdentifier=keyid']);
      const signer = certificate(dir, 'signer', 'issuing-ca', [
        'basicConstraints=critical,CA:FALSE', 'keyUsage=critical,digitalSignature',
      ]);
      const root = `did:web:${uid('root')}.example`;
      const tao = `did:web:${uid('tao')}.example`;
      const ti = `did:web:${uid('ti')}.example`;
      for (const rec of [
        { did: root, role: 'root_tao', credentialTypes: ['*'], status: 'active' },
        {
          did: tao, role: 'tao', credentialTypes: ['MembershipCredential'], accreditedBy: root,
          status: 'active', x509Certificates: [rootCa.der],
        },
        {
          did: ti, role: 'ti', credentialTypes: ['MembershipCredential'], accreditedBy: tao,
          status: 'active', x509Certificates: [issuingCa.der],
        },
      ]) {
        const put = await tir.put(rec);
        expect(put.status, put.text).toBe(201);
      }
      const check = async () =>
        (
          await fetch(`${stack.registry.url}/v1/x509/check`, {
            method: 'POST',
            headers: { 'content-type': 'application/json' },
            body: JSON.stringify({ x5c: [signer.der, issuingCa.der], credentialType: 'MembershipCredential' }),
          })
        ).json();
      expect(await check()).toMatchObject({ trusted: true, chain: [ti, tao, root] });

      await gotoRoute(page, '/trusted-issuers');
      await expectLoaded(page);
      await rowFor(table(page, 'Trusted issuers'), ti).getByRole('button', { name: 'Suspend', exact: true }).click();
      await expect(rowFor(table(page, 'Trusted issuers'), ti)).toContainText(/suspended/i);
      expect(await check()).toMatchObject({ trusted: false, chain: [ti] });

      await tir.remove(ti);
      await tir.remove(tao);
      await tir.remove(root);
    } finally {
      fs.rmSync(dir, { recursive: true, force: true });
    }
  });
});
