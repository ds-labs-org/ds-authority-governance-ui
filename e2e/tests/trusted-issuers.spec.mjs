import {
  test, expect, uid, gotoRoute, pageHeading, expectLoaded, table, rowFor as row, dialog, errorAlert, expectColumns,
} from './fixtures.mjs';

const TABLE = 'Trusted issuers';

async function addIssuer(page, { did, name, role, types, accreditedBy }) {
  await page.getByRole('button', { name: 'Add trusted issuer', exact: true }).click();
  const dlg = dialog(page, 'Add trusted issuer');
  await expect(dlg).toBeVisible();
  await dlg.getByLabel('DID', { exact: true }).fill(did);
  await dlg.getByLabel('Name', { exact: true }).fill(name);
  await dlg.getByLabel('Role', { exact: true }).selectOption(role); // option values: root_tao | tao | ti
  await dlg.getByLabel('Credential types', { exact: true }).fill(types); // comma-separated
  if (accreditedBy) await dlg.getByLabel('Accredited by', { exact: true }).fill(accreditedBy);
  await dlg.getByRole('button', { name: 'Save', exact: true }).click();
  await expect(dlg).toBeHidden();
}

async function checkTrust(page, did, type) {
  const form = page.getByRole('form', { name: 'Check trust' });
  await form.getByLabel('Issuer DID', { exact: true }).fill(did);
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
    await expectColumns(tbl, ['DID', 'Name', 'Role', 'Credential types', 'Accredited by', 'Status']);
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
    await dlg.getByLabel('DID', { exact: true }).fill(ti);
    await dlg.getByLabel('Role', { exact: true }).selectOption('ti');
    await dlg.getByLabel('Credential types', { exact: true }).fill('OtherCredential');
    await dlg.getByLabel('Accredited by', { exact: true }).fill(root);
    await dlg.getByRole('button', { name: 'Save', exact: true }).click();
    await expect(errorAlert(page, 'Could not save trusted issuer')).toBeVisible();
    await expect(dlg).toBeVisible();
    expect((await tir.get(ti)).status).toBe(404);
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
