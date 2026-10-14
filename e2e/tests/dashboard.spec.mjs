import { test, expect, uid, gotoRoute, pageHeading, expectLoaded } from './fixtures.mjs';

const count = (n) => new RegExp(`(^|\\D)${n}(\\D|$)`);

test.describe('dashboard', () => {
  test('shows live counts and links to the pending approvals', async ({ page, holders, hub, stack }) => {
    // Make sure there is at least one pending holder.
    const h = await holders.create(uid('dash'));
    expect((await holders.requestCredential(h.name, { issuerUrl: stack.hub.url })).status).toBe(201);

    const [defs, pending, issued] = await Promise.all([
      hub.definitions(),
      hub.holders({ status: 'pending' }),
      hub.credentials(),
    ]);
    expect(pending.length).toBeGreaterThan(0);

    await gotoRoute(page, '/');
    await expect(pageHeading(page, 'Dashboard')).toBeVisible();
    await expectLoaded(page);
    await expect(page.getByRole('region', { name: 'Credential definitions' })).toContainText(count(defs.length));
    await expect(page.getByRole('region', { name: 'Pending holders' })).toContainText(count(pending.length));
    await expect(page.getByRole('region', { name: 'Issued credentials' })).toContainText(count(issued.length));

    await page.getByRole('link', { name: 'Review pending approvals' }).click();
    await expect(page).toHaveURL(/#\/holders$/);
    await expect(pageHeading(page, 'Holders')).toBeVisible();
    await expect(page.getByRole('table', { name: 'Pending approval' }).getByRole('row').filter({ hasText: h.did }))
      .toBeVisible();
  });
});
