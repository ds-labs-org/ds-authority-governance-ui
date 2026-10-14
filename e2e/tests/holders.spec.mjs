// Holder approval queue and the issuance lifecycle that hangs off it:
// fake DCP holder -> pending -> approve/reject -> delivery -> revoke.
import {
  test, expect, uid, gotoRoute, pageHeading, expectLoaded, table, row, dialog, errorAlert,
  expectColumns, refreshUntil,
} from './fixtures.mjs';
import { fetchRevocationList, bitIsSet } from '../harness/backend.mjs';

const PENDING = 'Pending approval';
const ALL = 'All holders';
const ISSUED = 'Issued credentials';
const PROCESSES = 'Issuance processes';

/** A fresh holder identity with one Credential Request already sent. */
async function pendingHolder({ holders, stack }) {
  const h = await holders.create(uid('holder'));
  const req = await holders.requestCredential(h.name, { issuerUrl: stack.hub.url });
  expect(req.status, `credential request: ${req.body}`).toBe(201);
  expect(req.location).toBe(`/requests/${req.id}`);
  return { h, req };
}

async function openHolders(page) {
  await gotoRoute(page, '/holders');
  await expect(pageHeading(page, 'Holders')).toBeVisible();
  await expectLoaded(page);
}

test.describe('holders', () => {
  test('approve a pending holder; the credential is delivered, then revoked', async ({
    page, holders, hub, stack,
  }) => {
    const { h, req } = await pendingHolder({ holders, stack });

    // Manual approval: the request waits.
    expect((await holders.requestStatus(h.name, stack.hub.url, req.id)).body.status).toBe('RECEIVED');
    expect(await holders.deliveries(h.name)).toHaveLength(0);

    await openHolders(page);
    const pending = table(page, PENDING);
    await expectColumns(pending, ['DID', 'Name', 'Requested']);
    await expect(row(pending, h.did)).toBeVisible();
    await row(pending, h.did).getByRole('button', { name: 'Approve', exact: true }).click();
    await expect(row(table(page, PENDING), h.did)).toHaveCount(0);
    await expect(row(table(page, ALL), h.did)).toContainText(/approved/i);

    // Credentials page: process DELIVERED, credential active.
    await page.getByRole('navigation').getByRole('link', { name: 'Credentials', exact: true }).click();
    await expect(pageHeading(page, 'Credentials')).toBeVisible();
    await expectColumns(table(page, PROCESSES), ['ID', 'Holder', 'Credential definitions', 'State']);
    await refreshUntil(page, async () => {
      await expect(row(table(page, PROCESSES), req.id)).toContainText('DELIVERED');
    });
    const procRow = row(table(page, PROCESSES), req.id);
    await expect(procRow).toContainText(h.did);
    await expect(procRow).toContainText('membership-credential');

    const issued = table(page, ISSUED);
    await expectColumns(issued, ['ID', 'Type', 'Holder', 'Issued', 'Expires', 'Status']);
    await expect(row(issued, h.did)).toContainText('MembershipCredential');
    await expect(row(issued, h.did)).toContainText(/active/i);

    // The fake holder really received it (Storage API), signed by the hub.
    const deliveries = await holders.deliveries(h.name);
    expect(deliveries).toHaveLength(1);
    expect(deliveries[0].message).toMatchObject({
      type: 'CredentialMessage',
      issuerPid: req.id,
      holderPid: req.holderPid,
      status: 'ISSUED',
    });
    expect(deliveries[0].message.credentials).toHaveLength(1);
    expect(deliveries[0].tokenVerified, deliveries[0].tokenError ?? '').toBe(true);
    expect((await holders.requestStatus(h.name, stack.hub.url, req.id)).body.status).toBe('ISSUED');

    // Revoke through the UI.
    const [record] = await hub.credentials({ holderDid: h.did });
    expect(record).toMatchObject({ status: 'active', issuanceProcessId: req.id });
    expect(bitIsSet((await fetchRevocationList(stack)).bits, record.statusListIndex)).toBe(false);

    await row(issued, h.did).getByRole('button', { name: 'Revoke', exact: true }).click();
    const dlg = dialog(page, 'Revoke credential');
    await expect(dlg).toBeVisible();
    await dlg.getByRole('button', { name: 'Revoke', exact: true }).click();
    await expect(dlg).toBeHidden();
    await expect(row(table(page, ISSUED), h.did)).toContainText(/revoked/i);
    await expect(
      row(table(page, ISSUED), h.did).getByRole('button', { name: 'Revoke', exact: true }).and(page.locator(':enabled')),
    ).toHaveCount(0);

    // The public status list has the bit set.
    const list = await fetchRevocationList(stack);
    expect(list.contentType).toContain('application/vc+jwt');
    expect(list.bits.length).toBe(16 * 1024);
    expect(bitIsSet(list.bits, record.statusListIndex)).toBe(true);
    expect((await hub.credentials({ holderDid: h.did }))[0].status).toBe('revoked');
  });

  test('reject a pending holder; its process is REJECTED and further requests get 403', async ({
    page, holders, hub, stack,
  }) => {
    const { h, req } = await pendingHolder({ holders, stack });

    await openHolders(page);
    await row(table(page, PENDING), h.did).getByRole('button', { name: 'Reject', exact: true }).click();
    const dlg = dialog(page, 'Reject holder');
    await expect(dlg).toContainText(h.did);
    await dlg.getByRole('button', { name: 'Reject', exact: true }).click();
    await expect(dlg).toBeHidden();
    await expect(row(table(page, PENDING), h.did)).toHaveCount(0);
    await expect(row(table(page, ALL), h.did)).toContainText(/rejected/i);

    await gotoRoute(page, '/credentials');
    await expectLoaded(page);
    await expect(row(table(page, PROCESSES), req.id)).toContainText('REJECTED');
    await expect(row(table(page, ISSUED), h.did)).toHaveCount(0);

    expect((await holders.requestStatus(h.name, stack.hub.url, req.id)).body.status).toBe('REJECTED');
    expect(await holders.deliveries(h.name)).toHaveLength(0);

    const again = await holders.requestCredential(h.name, { issuerUrl: stack.hub.url });
    expect(again.status).toBe(403);
    expect(await hub.processes({ holderDid: h.did })).toHaveLength(1);
  });

  test('register a holder manually: listed as approved, never pending', async ({ page, hub }) => {
    const did = `did:web:${uid('acme')}.example`;
    const name = `ACME ${uid('org')}`;

    await openHolders(page);
    await page.getByRole('button', { name: 'Register holder', exact: true }).click();
    const dlg = dialog(page, 'Register holder');
    await dlg.getByLabel('DID', { exact: true }).fill(did);
    await dlg.getByLabel('Name', { exact: true }).fill(name);
    await dlg.getByRole('button', { name: 'Register', exact: true }).click();
    await expect(dlg).toBeHidden();

    const all = table(page, ALL);
    await expectColumns(all, ['DID', 'Name', 'Status']);
    await expect(row(all, did)).toContainText(name);
    await expect(row(all, did)).toContainText(/approved/i);
    await expect(row(table(page, PENDING), did)).toHaveCount(0);
    expect(await hub.holderByDid(did)).toMatchObject({ did, name, status: 'approved' });

    // Same DID again -> 409 -> error alert.
    await page.getByRole('button', { name: 'Register holder', exact: true }).click();
    const again = dialog(page, 'Register holder');
    await again.getByLabel('DID', { exact: true }).fill(did);
    await again.getByRole('button', { name: 'Register', exact: true }).click();
    await expect(errorAlert(page, 'Could not register holder')).toBeVisible();
    await again.getByRole('button', { name: 'Cancel', exact: true }).click();
    await expect(row(table(page, ALL), did)).toHaveCount(1);
  });
});
