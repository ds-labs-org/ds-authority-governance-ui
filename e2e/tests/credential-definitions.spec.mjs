import {
  test, expect, uid, gotoRoute, pageHeading, expectLoaded, table, row, dialog, errorAlert, expectColumns,
} from './fixtures.mjs';

const TABLE = 'Credential definitions';
const typeName = () => `E2e${uid('').replace(/[^a-z0-9]/gi, '')}Credential`;

test.describe('credential definitions', () => {
  test.beforeEach(async ({ page }) => {
    await gotoRoute(page, '/credential-definitions');
    await expect(pageHeading(page, 'Credential Definitions')).toBeVisible();
    await expectLoaded(page);
  });

  test('lists the seeded membership-credential', async ({ page }) => {
    const tbl = table(page, TABLE);
    await expectColumns(tbl, ['ID', 'Credential type', 'Format', 'Validity']);
    const seeded = row(tbl, 'membership-credential');
    await expect(seeded).toContainText('MembershipCredential');
    await expect(seeded).toContainText('VC1_0_JWT');
  });

  test('creates a definition through the form dialog', async ({ page, hub }) => {
    const id = uid('def');
    const type = typeName();
    await page.getByRole('button', { name: 'Create credential definition' }).click();
    const dlg = dialog(page, 'Create credential definition');
    await expect(dlg).toBeVisible();
    await dlg.getByLabel('ID', { exact: true }).fill(id);
    await dlg.getByLabel('Credential type', { exact: true }).fill(type);
    await dlg.getByLabel('Validity (seconds)', { exact: true }).fill('7200');
    await dlg.getByRole('button', { name: 'Create', exact: true }).click();
    await expect(dlg).toBeHidden();

    const created = row(table(page, TABLE), id);
    await expect(created).toContainText(type);
    await expect(created).toContainText('7200');

    const stored = await hub.getDefinition(id);
    expect(stored.status).toBe(200);
    expect(stored.json).toMatchObject({ id, credentialType: type, validity: 7200, format: 'VC1_0_JWT' });
  });

  test('edits the validity of a definition', async ({ page, hub }) => {
    const id = uid('def');
    expect((await hub.createDefinition({ id, credentialType: typeName(), validity: 3600 })).status).toBe(201);
    await page.reload();
    await expectLoaded(page);

    await row(table(page, TABLE), id).getByRole('button', { name: 'Edit', exact: true }).click();
    const dlg = dialog(page, 'Edit credential definition');
    await expect(dlg).toBeVisible();
    // The id is the key: shown, not editable.
    await expect(dlg.getByLabel('ID', { exact: true })).toHaveValue(id);
    await expect(dlg.getByLabel('ID', { exact: true })).not.toBeEditable();
    await dlg.getByLabel('Validity (seconds)', { exact: true }).fill('600');
    await dlg.getByRole('button', { name: 'Save', exact: true }).click();
    await expect(dlg).toBeHidden();

    await expect(row(table(page, TABLE), id)).toContainText('600');
    expect((await hub.getDefinition(id)).json.validity).toBe(600);
  });

  test('a duplicate id shows an error alert and keeps the dialog open', async ({ page }) => {
    await page.getByRole('button', { name: 'Create credential definition' }).click();
    const dlg = dialog(page, 'Create credential definition');
    await dlg.getByLabel('ID', { exact: true }).fill('membership-credential');
    await dlg.getByLabel('Credential type', { exact: true }).fill('MembershipCredential');
    await dlg.getByRole('button', { name: 'Create', exact: true }).click();

    await expect(errorAlert(page, 'Could not create credential definition')).toBeVisible();
    await expect(dlg).toBeVisible();
    await dlg.getByRole('button', { name: 'Cancel', exact: true }).click();
    await expect(dlg).toBeHidden();
    await expect(row(table(page, TABLE), 'membership-credential')).toHaveCount(1);
  });

  test('deletes a definition after confirmation; cancel keeps it', async ({ page, hub }) => {
    const id = uid('def');
    expect((await hub.createDefinition({ id, credentialType: typeName() })).status).toBe(201);
    await page.reload();
    await expectLoaded(page);

    // Cancel first: nothing happens.
    await row(table(page, TABLE), id).getByRole('button', { name: 'Delete', exact: true }).click();
    let dlg = dialog(page, 'Delete credential definition');
    await expect(dlg).toContainText(id);
    await dlg.getByRole('button', { name: 'Cancel', exact: true }).click();
    await expect(dlg).toBeHidden();
    await expect(row(table(page, TABLE), id)).toHaveCount(1);

    // Confirm.
    await row(table(page, TABLE), id).getByRole('button', { name: 'Delete', exact: true }).click();
    dlg = dialog(page, 'Delete credential definition');
    await dlg.getByRole('button', { name: 'Delete', exact: true }).click();
    await expect(dlg).toBeHidden();
    await expect(row(table(page, TABLE), id)).toHaveCount(0);
    expect((await hub.getDefinition(id)).status).toBe(404);
  });
});
