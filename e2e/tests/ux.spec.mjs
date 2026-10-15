// UX basics, with the backends stubbed per test via page.route: empty
// states, spinners that go away, and errors surfaced as alerts.
import { test, expect, gotoRoute, pageHeading, expectLoaded, errorAlert } from './fixtures.mjs';

const isQuery = (kind) => (url) =>
  /^\/api\/issuer\/v1alpha\/participants\/[^/]+\//.test(url.pathname) && url.pathname.endsWith(`/${kind}/query`);
const isRegistryList = (url) => url.pathname === '/api/registry/v1/issuers';

const PAGES = [
  { route: '/credential-definitions', heading: 'Credential Definitions', stubs: ['credentialdefinitions'], empty: ['No credential definitions'] },
  { route: '/holders', heading: 'Holders', stubs: ['holders'], empty: ['No pending approvals', 'No holders'] },
  {
    route: '/credentials',
    heading: 'Credentials',
    stubs: ['credentials', 'issuanceprocesses'],
    empty: ['No issued credentials', 'No issuance processes'],
    // Each table loads on its own: both must report the failure.
    regions: ['Issued credentials', 'Issuance processes'],
  },
  { route: '/trusted-issuers', heading: 'Trusted Issuers', registry: true, empty: ['No trusted issuers'] },
];

async function stubAll(page, p, handler) {
  for (const kind of p.stubs ?? []) await page.route(isQuery(kind), handler);
  if (p.registry) await page.route(isRegistryList, handler);
}

/** A failed load never renders as an empty table, in any section. */
async function expectFailedLoad(page, p, message) {
  for (const text of p.empty) await expect(page.getByText(text, { exact: true })).toHaveCount(0);
  for (const name of p.regions ?? []) {
    const alert = page.getByRole('region', { name, exact: true }).getByRole('alert');
    await expect(message ? alert.filter({ hasText: message }) : alert, `alert in "${name}"`).toBeVisible();
  }
}

test.describe('UX basics', () => {
  for (const p of PAGES) {
    test(`${p.route}: empty state`, async ({ page }) => {
      await stubAll(page, p, (r) =>
        r.fulfill({ json: p.registry ? { version: 0, total: 0, items: [] } : [] }),
      );
      await gotoRoute(page, p.route);
      await expect(pageHeading(page, p.heading)).toBeVisible();
      await expectLoaded(page);
      for (const text of p.empty) await expect(page.getByText(text, { exact: true })).toBeVisible();
      await expect(page.getByRole('alert')).toHaveCount(0);
    });

    test(`${p.route}: a backend error is shown in an alert with its message`, async ({ page }) => {
      await stubAll(page, p, (r) => r.fulfill({ status: 500, json: { error: 'e2e: backend exploded' } }));
      await gotoRoute(page, p.route);
      await expect(pageHeading(page, p.heading)).toBeVisible();
      await expect(errorAlert(page, 'e2e: backend exploded')).toBeVisible();
      await expectLoaded(page);
      await expectFailedLoad(page, p, 'e2e: backend exploded');
    });

    test(`${p.route}: an unreachable backend is shown in an alert`, async ({ page }) => {
      await stubAll(page, p, (r) => r.abort('connectionrefused'));
      await gotoRoute(page, p.route);
      await expect(page.getByRole('alert').first()).toBeVisible();
      await expectLoaded(page);
      await expectFailedLoad(page, p);
    });

    test(`${p.route}: a spinner shows while loading and goes away`, async ({ page }) => {
      await stubAll(page, p, async (r) => {
        await new Promise((res) => setTimeout(res, 1500));
        await r.continue();
      });
      await gotoRoute(page, p.route);
      await expect(page.getByRole('progressbar').first()).toBeVisible();
      await expectLoaded(page);
      await expect(page.getByRole('alert')).toHaveCount(0);
    });
  }

  test('dashboard: a failing count is an alert, not a silent zero', async ({ page }) => {
    await page.route(isQuery('holders'), (r) => r.fulfill({ status: 503, json: { error: 'e2e: hub unavailable' } }));
    await gotoRoute(page, '/');
    await expect(pageHeading(page, 'Dashboard')).toBeVisible();
    await expect(errorAlert(page, 'e2e: hub unavailable')).toBeVisible();
    await expectLoaded(page);
  });

  test('a gateway 401 (expired session) is reported, not swallowed', async ({ page }) => {
    await page.route(isQuery('credentialdefinitions'), (r) =>
      r.fulfill({ status: 401, json: { error: 'missing or invalid admin token' } }),
    );
    await gotoRoute(page, '/credential-definitions');
    await expect(errorAlert(page, 'missing or invalid admin token')).toBeVisible();
  });
});
