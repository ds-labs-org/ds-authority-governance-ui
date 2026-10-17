import {
  test, expect, uid, gotoRoute, pageHeading, expectLoaded, table, rowFor as row, dialog, errorAlert, expectColumns,
} from './fixtures.mjs';

const TABLE = 'Trusted issuers';

async function addIssuer(page, { did, name, role, types, accreditedBy }) {
  await page.getByRole('button', { name: 'Add trusted issuer', exact: true }).click();
  const dlg = dialog(page, 'Add trusted issuer');
  await expect(dlg).toBeVisible();
  await dlg.getByLabel('Issuer id', { exact: true }).fill(did);
  await dlg.getByLabel('Name', { exact: true }).fill(name);
  await dlg.getByLabel('Role', { exact: true }).selectOption(role); // option values: root_tao | tao | ti
  await dlg.getByLabel('Credential types', { exact: true }).fill(types); // comma-separated
  if (accreditedBy) await dlg.getByLabel('Accredited by', { exact: true }).fill(accreditedBy);
  await dlg.getByRole('button', { name: 'Save', exact: true }).click();
  await expect(dlg).toBeHidden();
}

async function checkTrust(page, did, type) {
  const form = page.getByRole('form', { name: 'Check trust' });
  await form.getByLabel('Issuer id', { exact: true }).fill(did);
  await form.getByLabel('Credential type', { exact: true }).fill(type);
  await form.getByRole('button', { name: 'Check', exact: true }).click();
  return form.getByRole('status');
}

test.describe('trusted issuers', () => {
  test.beforeEach(async ({ page }) => {
    await gotoRoute(page, '/trusted-issuers');
    await expect(pageHeading(page, 'Trusted Issuers')).toBeVisible();
    await expectLoaded(page);
  });

  test('root TAO -> accredited issuer -> trust check, suspend, protected delete', async ({ page, tir }) => {
    const root = `did:web:${uid('root')}.example`;
    const ti = `did:web:${uid('ti')}.example`;

    await expect(page.getByText(/read-only/i)).toHaveCount(0);

    await addIssuer(page, { did: root, name: 'E2E Authority', role: 'root_tao', types: '*' });
    const tbl = table(page, TABLE);
    await expectColumns(tbl, ['Issuer id', 'Name', 'Role', 'Credential types', 'Accredited by', 'Status']);
    await expect(row(tbl, root)).toContainText('Root TAO');
    await expect(row(tbl, root)).toContainText('*');
    await expect(row(tbl, root)).toContainText(/active/i);

    await addIssuer(page, {
      did: ti, name: 'E2E Issuer', role: 'ti', types: 'MembershipCredential', accreditedBy: root,
    });
    const tiRow = row(table(page, TABLE), ti);
    await expect(tiRow).toContainText('Trusted issuer');
    await expect(tiRow).toContainText('MembershipCredential');
    await expect(tiRow).toContainText(root);
    expect((await tir.get(ti)).json).toMatchObject({ role: 'ti', accreditedBy: root, status: 'active' });

    // Trusted, with the chain ti -> root.
    let result = await checkTrust(page, ti, 'MembershipCredential');
    await expect(result).toContainText('Trusted');
    await expect(result).not.toContainText('Not trusted');
    const chain = page.getByRole('form', { name: 'Check trust' }).getByRole('list', { name: 'Trust chain' });
    await expect(chain.getByRole('listitem')).toHaveCount(2);
    await expect(chain.getByRole('listitem').nth(0)).toContainText(ti);
    await expect(chain.getByRole('listitem').nth(1)).toContainText(root);

    // Not trusted for a type outside its scope.
    result = await checkTrust(page, ti, 'OtherCredential');
    await expect(result).toContainText('Not trusted');

    // Suspend the root: the whole chain stops being trusted.
    await row(table(page, TABLE), root).getByRole('button', { name: 'Suspend', exact: true }).click();
    await expect(row(table(page, TABLE), root)).toContainText(/suspended/i);
    await expect(row(table(page, TABLE), root).getByRole('button', { name: 'Activate', exact: true })).toBeVisible();
    expect((await tir.get(root)).json.status).toBe('suspended');
    result = await checkTrust(page, ti, 'MembershipCredential');
    // The previous answer (OtherCredential) was also "Not trusted": wait for
    // the new one, whose reason names the suspended root.
    await expect(result).toContainText(`'${root}' is not active`);
    await expect(result).toContainText('Not trusted');

    // Deleting a record that still accredits someone: 409 -> error alert, nothing removed.
    await row(table(page, TABLE), root).getByRole('button', { name: 'Delete', exact: true }).click();
    const dlg = dialog(page, 'Delete trusted issuer');
    await expect(dlg).toContainText(root);
    await dlg.getByRole('button', { name: 'Delete', exact: true }).click();
    await expect(errorAlert(page, 'Could not delete trusted issuer')).toBeVisible();
    await expect(row(table(page, TABLE), root)).toHaveCount(1);
    expect((await tir.get(root)).status).toBe(200);

    // Reactivate: trusted again.
    await row(table(page, TABLE), root).getByRole('button', { name: 'Activate', exact: true }).click();
    await expect(row(table(page, TABLE), root)).toContainText(/active/i);
    result = await checkTrust(page, ti, 'MembershipCredential');
    // A positive wait first: a negative assertion alone also passes while checking.
    await expect(result).toContainText('Trusted');
    await expect(result).not.toContainText('Not trusted');

    // Cleanup through the backend (keeps the shared registry small).
    await tir.remove(ti);
    await tir.remove(root);
  });

  test('deleting a leaf issuer after confirmation removes it', async ({ page, tir }) => {
    const root = `did:web:${uid('root')}.example`;
    expect((await tir.put({ did: root, role: 'root_tao', credentialTypes: ['*'], status: 'active' })).status).toBe(201);
    await page.reload();
    await expectLoaded(page);

    await row(table(page, TABLE), root).getByRole('button', { name: 'Delete', exact: true }).click();
    await dialog(page, 'Delete trusted issuer').getByRole('button', { name: 'Delete', exact: true }).click();
    await expect(row(table(page, TABLE), root)).toHaveCount(0);
    expect((await tir.get(root)).status).toBe(404);
  });

  test('an invalid accreditation (TI scoped wider than its accreditor) shows an error', async ({ page, tir }) => {
    const root = `did:web:${uid('root')}.example`;
    expect(
      (await tir.put({ did: root, role: 'root_tao', credentialTypes: ['MembershipCredential'], status: 'active' }))
        .status,
    ).toBe(201);
    await page.reload();
    await expectLoaded(page);

    await page.getByRole('button', { name: 'Add trusted issuer', exact: true }).click();
    const dlg = dialog(page, 'Add trusted issuer');
    const ti = `did:web:${uid('ti')}.example`;
    await dlg.getByLabel('Issuer id', { exact: true }).fill(ti);
    await dlg.getByLabel('Role', { exact: true }).selectOption('ti');
    await dlg.getByLabel('Credential types', { exact: true }).fill('OtherCredential');
    await dlg.getByLabel('Accredited by', { exact: true }).fill(root);
    await dlg.getByRole('button', { name: 'Save', exact: true }).click();
    await expect(errorAlert(page, 'Could not save trusted issuer')).toBeVisible();
    await expect(dlg).toBeVisible();
    expect((await tir.get(ti)).status).toBe(404);
    await tir.remove(root);
  });

  test('an OID4VCI issuer (URL id, X.509 anchor) next to DCP ones: filter, accreditations, audit', async ({ page, tir }) => {
    const root = `did:web:${uid('root')}.example`;
    const oid4vci = `https://${uid('mdl')}.example/tenants/acme`;
    expect((await tir.put({ id: root, role: 'root_tao', credentialTypes: ['*'], status: 'active' })).status).toBe(201);
    await page.reload();
    await expectLoaded(page);

    await page.getByRole('button', { name: 'Add trusted issuer', exact: true }).click();
    const dlg = dialog(page, 'Add trusted issuer');
    await dlg.getByLabel('Issuer id', { exact: true }).fill(oid4vci);
    await dlg.getByLabel('Name', { exact: true }).fill('E2E mDL issuer');
    await dlg.getByLabel('Role', { exact: true }).selectOption('ti');
    await dlg.getByLabel('Credential types', { exact: true }).fill('org.iso.18013.5.1.mDL');
    await dlg.getByLabel('Accredited by', { exact: true }).fill(root);
    // A PEM header is not one base64 DER certificate per line.
    await dlg.getByLabel('X.509 trust anchors', { exact: true }).fill('-----BEGIN CERTIFICATE-----');
    await dlg.getByRole('button', { name: 'Save', exact: true }).click();
    await expect(dlg).toContainText('One base64 DER certificate per line');
    await dlg.getByLabel('X.509 trust anchors', { exact: true }).fill('');
    await dlg.getByRole('button', { name: 'Save', exact: true }).click();
    await expect(dlg).toBeHidden();
    expect((await tir.get(oid4vci)).json).toMatchObject({ id: oid4vci, role: 'ti', accreditedBy: root });

    // "URL (OID4VCI)" lists it and not the DID-named root.
    await page.getByLabel('Filter by identifier', { exact: true }).selectOption('url');
    await expect(row(table(page, TABLE), oid4vci)).toHaveCount(1);
    await expect(row(table(page, TABLE), root)).toHaveCount(0);
    await page.getByLabel('Filter by identifier', { exact: true }).selectOption('');

    // Its accreditation chain as VCs: the issuer's and the root's.
    await row(table(page, TABLE), oid4vci).getByRole('button', { name: 'Accreditations', exact: true }).click();
    const acc = dialog(page, 'Accreditations');
    await expect(acc).toContainText('2 accreditation VCs');
    await acc.getByRole('button', { name: 'Close', exact: true }).click();

    // The write is in the audit log.
    const audit = table(page, 'Audit log');
    await expect(audit.getByRole('row').filter({ hasText: oid4vci }).first()).toContainText('put');
    expect((await tir.audit()).json.entries.some((e) => e.id === oid4vci)).toBe(true);

    await tir.remove(oid4vci);
    await tir.remove(root);
  });

  test('a postal address is kept, checked and puts the party on the LoTE', async ({ page, tir }) => {
    const root = `did:web:${uid('root')}.example`;
    const ti = `did:web:${uid('ti')}.example`;
    expect((await tir.put({ id: root, role: 'root_tao', credentialTypes: ['*'], status: 'active' })).status).toBe(201);
    expect(
      (await tir.put({ id: ti, role: 'ti', credentialTypes: ['MembershipCredential'], accreditedBy: root, status: 'active' }))
        .status,
    ).toBe(201);
    const lotedIds = async () => {
      const res = await page.request.get(new URL('/api/registry/v1/lote', page.url()).href);
      expect(res.status()).toBe(200);
      const lote = await res.json();
      return lote.LoTE.TrustedEntitiesList.flatMap((e) =>
        e.TrustedEntityServices.flatMap((s) => s.ServiceInformation.ServiceDigitalIdentity.OtherIds ?? []),
      );
    };
    // No address yet, so not on the LoTE.
    expect(await lotedIds()).not.toContain(ti);
    await page.reload();
    await expectLoaded(page);

    await row(table(page, TABLE), ti).getByRole('button', { name: 'Edit', exact: true }).click();
    const dlg = dialog(page, 'Edit trusted issuer');
    await dlg.getByLabel('Locality', { exact: true }).fill('Lyon');
    await dlg.getByRole('button', { name: 'Save', exact: true }).click();
    await expect(dlg).toContainText('Street, locality and country are all required');
    await dlg.getByLabel('Street', { exact: true }).fill('1 Rue de la République');
    await dlg.getByLabel('Country', { exact: true }).fill('France');
    await dlg.getByRole('button', { name: 'Save', exact: true }).click();
    await expect(dlg).toContainText('two-letter ISO 3166-1 code');
    await dlg.getByLabel('Country', { exact: true }).fill('fr');
    await dlg.getByRole('button', { name: 'Save', exact: true }).click();
    await expect(dlg).toBeHidden();
    expect((await tir.get(ti)).json.postalAddress).toEqual({
      streetAddress: '1 Rue de la République',
      locality: 'Lyon',
      country: 'FR',
    });
    expect(await lotedIds()).toContain(ti);

    // Editing again starts from the stored address.
    await row(table(page, TABLE), ti).getByRole('button', { name: 'Edit', exact: true }).click();
    await expect(dialog(page, 'Edit trusted issuer').getByLabel('Locality', { exact: true })).toHaveValue('Lyon');
    await dialog(page, 'Edit trusted issuer').getByRole('button', { name: 'Cancel', exact: true }).click();

    await tir.remove(ti);
    await tir.remove(root);
  });

  for (const status of [401, 403]) {
    test(`read-only banner when the registry admin API answers ${status}`, async ({ page }) => {
      await page.route(
        (url) => url.pathname === '/api/registry/v1/admin',
        (r) => r.fulfill({ status, json: { error: 'admin API not available' } }),
      );
      await page.reload();
      await expect(pageHeading(page, 'Trusted Issuers')).toBeVisible();
      await expect(page.getByText(/read-only/i).first()).toBeVisible();
      // No write controls offered (hidden or disabled).
      for (const name of ['Add trusted issuer', 'Suspend', 'Activate', 'Delete']) {
        await expect(
          page.getByRole('button', { name, exact: true }).and(page.locator(':enabled')),
          `"${name}" must not be usable read-only`,
        ).toHaveCount(0);
      }
      // Reads still work: the check form is there.
      await expect(page.getByRole('form', { name: 'Check trust' })).toBeVisible();
    });
  }
});
