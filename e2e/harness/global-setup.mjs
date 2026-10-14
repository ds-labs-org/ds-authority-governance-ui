// Playwright globalSetup: start the stack once for the whole run (workers=1,
// shared state). The stop handle lives on globalThis because globalSetup and
// globalTeardown run in the same runner process.
import { startStack } from './stack.mjs';

export default async function globalSetup() {
  const { info, stop } = await startStack({ log: (m) => console.log(`[stack] ${m}`) });
  globalThis.__E2E_STACK_STOP__ = stop;
  process.env.E2E_UI_URL = info.uiUrl;
}
