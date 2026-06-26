import { describe, expect, test } from 'vitest';
import { page, serverUrl, waitForBuildStable } from '~utils';

// A lazy-compiled module that throws during init must be catchable at the
// consumer's `await import(...)` — and stay catchable across a page refresh.
//
// The two loads exercise two different proxy code paths against the SAME dev
// engine (only the browser page reloads):
//   1. cold  — first compile, served through the on-demand `@vite/lazy` response
//   2. warm  — after the engine rebuilds main.js around the now-"fetched" proxy
//
// Either way the result is identical: caught at `await import(...)`, with no
// unhandled rejection. Regression test for vitejs/vite#21626 / rolldown#9975.
const lines = (text: string | null) => (text ?? '').split('\n').filter(Boolean);

async function clickAndExpectCaught() {
  await page.click('#lazy-init-error-catch-btn');
  await expect.poll(() => page.textContent('#lazy-init-error-status')).toBe('catch-done');

  // The consumer's try/catch sees the init error...
  expect(await page.textContent('#lazy-init-error-log')).toContain('caught: boom during lazy init');

  // ...and because it was handled, NOTHING escapes as an unhandled rejection.
  // Give any stray rejection a chance to fire before asserting none did.
  await page.waitForTimeout(100);
  expect(
    lines(await page.textContent('#lazy-init-error-unhandled')),
    'try/catch handled the init error, so no unhandled rejection should fire',
  ).toEqual([]);
}

describe('lazy-compilation: lazy-init-error (try/catch)', () => {
  test('init error is catchable on first compile and after a refresh', async () => {
    await page.goto(serverUrl, { waitUntil: 'domcontentloaded' });

    // 1. Cold: first compile.
    await clickAndExpectCaught();

    // Let the rebuild triggered by the lazy compile settle, then refresh the
    // page WITHOUT restarting the dev engine — main.js now uses the fetched proxy.
    await waitForBuildStable();
    await page.reload({ waitUntil: 'domcontentloaded' });

    // 2. Warm: fetched-proxy path. Same result.
    await clickAndExpectCaught();
  });
});
