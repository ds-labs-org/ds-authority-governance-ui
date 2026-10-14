import { test, expect, NAV, gotoRoute, pageHeading, errorAlert } from './fixtures.mjs';

test.describe('app shell', () => {
  test('loads and shows the navigation with every section', async ({ page }) => {
    await gotoRoute(page, '/');
    const nav = page.getByRole('navigation');
    for (const [name] of NAV) {
      await expect(nav.getByRole('link', { name, exact: true })).toBeVisible();
    }
  });

  test('each nav link opens its page with the matching h1', async ({ page }) => {
    await gotoRoute(page, '/');
    const nav = page.getByRole('navigation');
    for (const [name, route] of NAV) {
      await nav.getByRole('link', { name, exact: true }).click();
      await expect(page).toHaveURL(new RegExp(`#${route.replace(/\//g, '\\/')}$`));
      await expect(pageHeading(page, name)).toBeVisible();
    }
  });

  for (const [name, route] of NAV) {
    test(`deep link ${route} shows "${name}"`, async ({ page }) => {
      await gotoRoute(page, route);
      await expect(pageHeading(page, name)).toBeVisible();
    });
  }

  test('a broken configuration.json shows an error alert instead of the app', async ({ page }) => {
    await page.route('**/configuration.json', (r) =>
      r.fulfill({ status: 200, contentType: 'application/json', body: '{"identity_api_path": ' }),
    );
    await gotoRoute(page, '/');
    await expect(errorAlert(page, 'Could not load configuration.json')).toBeVisible();
    await expect(page.getByRole('navigation')).toHaveCount(0);
    await expect(page.getByRole('progressbar')).toHaveCount(0);
  });

  test('a missing configuration.json (404) shows the same error alert', async ({ page }) => {
    await page.route('**/configuration.json', (r) => r.fulfill({ status: 404, body: 'not found' }));
    await gotoRoute(page, '/');
    await expect(errorAlert(page, 'Could not load configuration.json')).toBeVisible();
  });

  test('registry_api_path and participant_context_id are optional in configuration.json', async ({ page }) => {
    await page.route('**/configuration.json', (r) =>
      r.fulfill({
        json: { identity_api_path: '/api/identity', issuer_admin_api_path: '/api/issuer', bearer_token: null },
      }),
    );
    await gotoRoute(page, '/credential-definitions');
    await expect(pageHeading(page, 'Credential Definitions')).toBeVisible();
    await expect(page.getByRole('row').filter({ hasText: 'membership-credential' })).toBeVisible();
  });

  test('bearer_token from configuration.json is sent on admin API calls', async ({ page, stack }) => {
    await page.route('**/configuration.json', (r) =>
      r.fulfill({ json: { ...stack.uiConfig, bearer_token: 'e2e-config-token' } }),
    );
    const apiCall = page.waitForRequest((req) => new URL(req.url()).pathname.startsWith('/api/issuer/'));
    await gotoRoute(page, '/credential-definitions');
    expect((await apiCall).headers()['authorization']).toBe('Bearer e2e-config-token');
  });
});
