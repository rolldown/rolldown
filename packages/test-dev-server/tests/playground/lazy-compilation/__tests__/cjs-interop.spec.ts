import { describe, expect, test } from 'vitest';
import { page, serverUrl, waitForBuildStable } from '~utils';

// Regression for vitejs/vite#23558: `import()` of a CommonJS module resolved
// to the raw `module.exports`, so `mod.default` was undefined.
describe('lazy-compilation: cjs-interop', () => {
  // `retry: 0`: only the first click of a fresh page takes the cold lazy path.
  test('import() of a CommonJS module has a default export', { retry: 0 }, async () => {
    await page.goto(serverUrl, { waitUntil: 'domcontentloaded' });
    await waitForBuildStable();
    await page.click('#cjs-interop-btn');
    await expect.poll(() => page.textContent('#cjs-interop-status')).toBe('done');

    const log = (await page.textContent('#cjs-interop-log')) ?? '';
    // Compiled on demand.
    expect(log).toContain('fetched.default = fetched-1');
    expect(log).toContain('fetched.version = fetched-1');
    // Already in the page through a static import.
    expect(log).toContain('resident.default = resident-1');
    expect(log).toContain('resident.version = resident-1');
    expect(log).toContain('resident.default === static import: true');
    expect(log).not.toContain('UNDEFINED');
  });
});
