// Playwright globalTeardown: stop everything globalSetup started.
import fs from 'node:fs';
import { STACK_FILE } from './stack.mjs';

export default async function globalTeardown() {
  const stop = globalThis.__E2E_STACK_STOP__;
  if (stop) {
    await stop();
    delete globalThis.__E2E_STACK_STOP__;
    return;
  }
  // Fallback (setup ran elsewhere): kill the recorded backend pids.
  try {
    const { pids = [] } = JSON.parse(fs.readFileSync(STACK_FILE, 'utf8'));
    for (const pid of pids) {
      try {
        process.kill(pid, 'SIGTERM');
      } catch {
        /* already gone */
      }
    }
  } catch {
    /* no stack file */
  }
}
