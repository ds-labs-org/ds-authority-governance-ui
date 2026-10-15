import { test, expect, uid, gotoRoute, pageHeading, expectLoaded } from './fixtures.mjs';

/** The card's main number (not any digit in the card, e.g. "0 active"). */
const countIn = (page, region) => page.getByRole('region', { name: region }).getByTestId('count');

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
    await expect(countIn(page, 'Credential definitions')).toHaveText(String(defs.length));
    await expect(countIn(page, 'Pending holders')).toHaveText(String(pending.length));
    await expect(countIn(page, 'Issued credentials')).toHaveText(String(issued.length));

    await page.getByRole('link', { name: 'Review pending approvals' }).click();
    await expect(page).toHaveURL(/#\/holders$/);
    await expect(pageHeading(page, 'Holders')).toBeVisible();
    await expect(page.getByRole('table', { name: 'Pending approval' }).getByRole('row').filter({ hasText: h.did }))
      .toBeVisible();
  });
});
