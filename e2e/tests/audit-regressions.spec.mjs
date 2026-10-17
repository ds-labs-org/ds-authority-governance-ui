// Non-regression checks for the 2026-10-16 project audit (findings C1, E1,
// E2, E3), against the real hub and registry binaries and, where the finding
// is visible there, through the console.
// E4 (the harness could not start the registry) has no spec of its own: the
// whole suite is its check.
import { spawn } from 'node:child_process';
import http from 'node:http';
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
async function startCredentialService(bin, trustedDid) {
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

test.describe('E2: a Credential Service holds and presents only valid credentials of its own', () => {
  test('the Storage API refuses foreign, expired and untrusted credentials; presentations drop expired and repeats', async ({
    stack,
  }) => {
    test.skip(!stack.hub.bin, 'needs the hub binary (HUB_URL is set)');
    const { parties, close } = await startParties(['issuer', 'rogue']);
    const cs = await startCredentialService(stack.hub.bin, parties.issuer.did);
    try {
      const vc = (issuer, subject, expFromNow) => {
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
      };
      const deliver = async (payload) => {
        const token = await parties.issuer.selfIssuedToken(cs.did);
        const res = await fetch(`${cs.url}/credentials`, {
          method: 'POST',
          headers: { authorization: `Bearer ${token}`, 'content-type': 'application/json' },
          body: JSON.stringify({
            '@context': ['https://w3id.org/dspace-dcp/v1.0/dcp.jsonld'],
            type: 'CredentialMessage',
            issuerPid: 'issuer-pid', holderPid: 'holder-pid', status: 'ISSUED',
            credentials: [{ credentialType: 'MembershipCredential', payload, format: 'jwt' }],
          }),
        });
        return res.status;
      };

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
