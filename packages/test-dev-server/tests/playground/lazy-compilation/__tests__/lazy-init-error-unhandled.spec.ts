import { describe, expect, test } from 'vitest';
import { page, serverUrl, waitForBuildStable } from '~utils';

// Mirror of lazy-init-error.spec.ts for the NO-handler case. A lazy module that
// throws during init, dynamically imported without a try/catch, must surface as
// an unhandled rejection — and keep doing so across a page refresh.
//
// Two loads, same dev engine (only the browser page reloads):
//   1. cold  — first compile, via the on-demand `@vite/lazy` response
//   2. warm  — after the engine rebuilds main.js around the "fetched" proxy
//
// The bug this guards (vitejs/vite#21626 / rolldown#9975) made the cold path
// swallow the error (no rejection at all); the warm path was already correct.
// Pinning both keeps them consistent.
const lines = (text: string | null) => (text ?? '').split('\n').filter(Boolean);

async function clickAndExpectUnhandled() {
  await page.click('#lazy-init-error-nocatch-btn');
  await expect.poll(() => page.textContent('#lazy-init-error-status')).toBe('nocatch-done');
  await expect
    .poll(async () => lines(await page.textContent('#lazy-init-error-unhandled')))
    .toEqual(['boom during lazy init']);
}

describe('lazy-compilation: lazy-init-error (no handler)', () => {
  test('init error surfaces as an unhandled rejection on first compile and after a refresh', async () => {
    await page.goto(serverUrl, { waitUntil: 'domcontentloaded' });
    await waitForBuildStable();

    // 1. Cold: first compile.
    await clickAndExpectUnhandled();

    // Settle the rebuild, then refresh WITHOUT restarting the dev engine. The
    // reload resets the DOM, so the unhandled log starts empty again.
    await waitForBuildStable();
    await page.reload({ waitUntil: 'domcontentloaded' });
    expect(lines(await page.textContent('#lazy-init-error-unhandled'))).toEqual([]);

    // 2. Warm: fetched-proxy path. The unhandled rejection surfaces again.
    await clickAndExpectUnhandled();
  });
});
