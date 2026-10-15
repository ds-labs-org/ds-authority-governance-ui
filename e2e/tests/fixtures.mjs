// Shared fixtures and UI helpers. Selectors follow the accessibility
// contract in ../README.md - change both together.
import { test as base, expect } from '@playwright/test';
import { randomUUID } from 'node:crypto';
import { readStack } from '../harness/stack.mjs';
import { fakeHolderClient } from '../harness/fake-holder.mjs';
import { hubAdmin, registry } from '../harness/backend.mjs';

export { expect };

export const test = base.extend({
  stack: [async ({}, use) => use(readStack()), { scope: 'worker' }],
  baseURL: async ({ stack }, use) => use(stack.uiUrl),
  /** Control client for the fake DCP holder server. */
  holders: async ({ stack }, use) => use(fakeHolderClient(stack.fakeHolder.url)),
  /** Hub admin API, for setup and assertions only. */
  hub: async ({ stack }, use) => use(hubAdmin(stack)),
  /** Registry API, for setup and assertions only. */
  tir: async ({ stack }, use) => use(registry(stack)),
});

/** Unique, lowercase, DNS/identifier-safe suffix. */
export const uid = (prefix = 'e2e') => `${prefix}-${randomUUID().slice(0, 8)}`;

export const NAV = [
  ['Dashboard', '/'],
  ['Credential Definitions', '/credential-definitions'],
  ['Holders', '/holders'],
  ['Credentials', '/credentials'],
  ['Trusted Issuers', '/trusted-issuers'],
  ['Identity Bootstrap', '/identity-bootstrap'],
];

/** Hash routing: `/holders` is served at `/#/holders`. */
export async function gotoRoute(page, route) {
  await page.goto(`/#${route}`);
}

/** The page's single h1. */
export const pageHeading = (page, name) => page.getByRole('heading', { level: 1, name, exact: true });

/** No loading spinner (role=progressbar) left on the page. */
export async function expectLoaded(page) {
  await expect(page.getByRole('progressbar')).toHaveCount(0);
}

/** A table by accessible name (aria-label or caption). */
export const table = (page, name) => page.getByRole('table', { name, exact: true });

/** The row of `tbl` containing `text`. */
export const row = (tbl, text) => tbl.getByRole('row').filter({ hasText: text });

const escapeRe = (s) => s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');

/** The row of `tbl` whose FIRST cell is exactly `key` (use when the key also
 * appears in other rows, e.g. a root DID in its children's "Accredited by"). */
export const rowFor = (tbl, key) =>
  tbl.getByRole('row').filter({
    has: tbl.page().locator('td:first-child, th:first-child').filter({
      hasText: new RegExp(`^\\s*${escapeRe(key)}\\s*$`),
    }),
  });

export const dialog = (page, name) => page.getByRole('dialog', { name, exact: true });

/** First alert containing `text` (a page may show the same error twice). */
export const errorAlert = (page, text) => page.getByRole('alert').filter({ hasText: text }).first();

/**
 * Clicks the page's "Refresh" button until `assertion` (an async fn of
 * `expect` checks) passes - the UI is not required to poll on its own.
 */
export async function refreshUntil(page, assertion, { timeout = 30_000 } = {}) {
  await expect(async () => {
    await page.getByRole('button', { name: 'Refresh', exact: true }).click();
    await expectLoaded(page);
    await assertion();
  }).toPass({ timeout, intervals: [300, 500, 1000] });
}

/** Columns a table must expose, in order, as `columnheader` cells. */
export async function expectColumns(tbl, names) {
  const headers = tbl.getByRole('columnheader');
  for (const n of names) {
    await expect(headers.filter({ hasText: n }).first(), `column "${n}"`).toBeVisible();
  }
}
