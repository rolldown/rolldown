import { describe, expect, test } from 'vitest';
import {
  editFile,
  page,
  plantReloadMarker,
  readReloadMarker,
  waitForBuildStable,
} from '~utils';

describe('hmr-accept-exports', () => {
  test('renders the initial value', async () => {
    await waitForBuildStable();
    await expect.poll(() => page.textContent('.value')).toBe('exports-v1');
    await expect.poll(() => page.textContent('.main-runs')).toBe('1');
  });

  test('an importer that reads only accepted exports is not re-run', async () => {
    await waitForBuildStable();
    await plantReloadMarker();

    editFile('app.js', (code) => code.replace("'exports-v1'", "'exports-v2'"));
    await expect.poll(() => page.textContent('.value')).toBe('exports-v2');

    expect(await readReloadMarker()).toBe('alive');
    expect(await page.textContent('.main-runs')).toBe('1');
    expect(await page.textContent('.main-saw')).toBe('exports-v1');
    await waitForBuildStable();
  });

  // needs the Vite client half of rolldown#10061 (`acceptExports` in `bundledDevHmrClient.ts`)
  test.skip('an importer that reads a non-accepted export reloads the page', async () => {
    await waitForBuildStable();
    await plantReloadMarker();
    editFile('main.js', (code) =>
      code
        .replace("import { value } from './app.js';", "import { value, other } from './app.js';")
        .replace('String(globalThis.__mainRuns);', 'String(globalThis.__mainRuns) + other;'),
    );
    await expect.poll(() => readReloadMarker()).toBe(null);
    await expect.poll(() => page.textContent('.main-runs')).toBe('1other');
    await waitForBuildStable();

    await plantReloadMarker();
    editFile('app.js', (code) => code.replace("'exports-v2'", "'exports-v3'"));
    await expect.poll(() => page.textContent('.value')).toBe('exports-v3');
    await expect.poll(() => readReloadMarker()).toBe(null);
    await expect.poll(() => page.textContent('.main-runs')).toBe('1other');
    await expect.poll(() => page.textContent('.main-saw')).toBe('exports-v3');
    await waitForBuildStable();
  });
});
