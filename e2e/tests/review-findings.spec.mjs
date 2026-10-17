// Adversarial review findings: each test pins a confirmed UI defect and is
// expected to FAIL until the defect is fixed.
import {
  test, expect, uid, gotoRoute, pageHeading, expectLoaded, table, row, rowFor, dialog,
} from './fixtures.mjs';

const delay = (ms) => new Promise((r) => setTimeout(r, ms));

test.describe('review findings', () => {
  test('keyboard: opening a dialog moves focus into it and Escape closes it', async ({ page }) => {
    await gotoRoute(page, '/holders');
    await expect(pageHeading(page, 'Holders')).toBeVisible();
    await expectLoaded(page);
    await page.getByRole('button', { name: 'Register holder', exact: true }).focus();
    await page.keyboard.press('Enter');
    const dlg = dialog(page, 'Register holder');
    await expect(dlg).toBeVisible();
    // Focus must land inside the modal (otherwise Tab walks the page behind it
    // and the dialog's Escape handler, bound on the modal box, never fires).
    expect(await page.evaluate(() => !!document.activeElement?.closest('[role="dialog"]'))).toBe(true);
    await page.keyboard.press('Escape');
    await expect(dlg).toBeHidden();
  });

  test('375px: the nav sidebar does not cover the page content', async ({ page }) => {
    await page.setViewportSize({ width: 375, height: 800 });
    await gotoRoute(page, '/holders');
    await expect(pageHeading(page, 'Holders')).toBeVisible();
    await expectLoaded(page);
    // The expanded sidebar overlays the content and intercepts the click.
    await page.getByRole('button', { name: 'Register holder', exact: true }).click({ timeout: 3_000 });
    await expect(dialog(page, 'Register holder')).toBeVisible();
  });

  test('a row action finishing does not close a dialog the user opened meanwhile', async ({ page, tir }) => {
    const root = `did:web:${uid('root')}.example`;
    expect((await tir.put({ did: root, role: 'root_tao', credentialTypes: ['*'], status: 'active' })).status).toBe(201);
    await page.route(
      (url) => url.pathname.startsWith('/api/registry/v1/issuers/') && url.pathname.endsWith('/status'),
      async (r) => {
        await delay(1500);
        await r.continue();
      },
    );
    await gotoRoute(page, '/trusted-issuers');
    await expectLoaded(page);
    await rowFor(table(page, 'Trusted issuers'), root).getByRole('button', { name: 'Suspend', exact: true }).click();
    // "Add trusted issuer" stays enabled while the suspend is in flight.
    await page.getByRole('button', { name: 'Add trusted issuer', exact: true }).click();
    const dlg = dialog(page, 'Add trusted issuer');
    await dlg.getByLabel('Issuer id', { exact: true }).fill('did:web:half-typed.example');
    await expect(rowFor(table(page, 'Trusted issuers'), root)).toContainText(/suspended/i);
    // The suspend's completion handler runs open.set(Open::None): typed input lost.
    await expect(dlg).toBeVisible();
    await expect(dlg.getByLabel('Issuer id', { exact: true })).toHaveValue('did:web:half-typed.example');
    await tir.remove(root);
  });

  test('approve stays disabled for a holder while its approval is in flight', async ({ page, holders, stack }) => {
    const a = await holders.create(uid('ha'));
    expect((await holders.requestCredential(a.name, { issuerUrl: stack.hub.url })).status).toBe(201);
    const b = await holders.create(uid('hb'));
    expect((await holders.requestCredential(b.name, { issuerUrl: stack.hub.url })).status).toBe(201);
    await page.route(
      (url) => url.pathname.endsWith('/approve'),
      async (r) => {
        await delay(2000);
        await r.continue();
      },
    );
    await gotoRoute(page, '/holders');
    await expectLoaded(page);
    const pending = table(page, 'Pending approval');
    await row(pending, a.did).getByRole('button', { name: 'Approve', exact: true }).click();
    await expect(row(pending, a.did).getByRole('button', { name: 'Approve', exact: true })).toBeDisabled();
    await row(pending, b.did).getByRole('button', { name: 'Approve', exact: true }).click();
    // `busy` is a single Option<holderId>: approving B re-enables A while A's
    // POST .../approve is still pending, so A can be approved twice.
    await expect(row(pending, a.did).getByRole('button', { name: 'Approve', exact: true })).toBeDisabled({
      timeout: 500,
    });
  });
});
