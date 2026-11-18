import { describe, expect, test } from 'vitest';
import { editFile, page, waitForBuildStable } from '~utils';

const plantMarker = () =>
  page.evaluate(() => ((window as unknown as { __marker?: string }).__marker = 'alive'));
const readMarker = () =>
  page.evaluate(() => (window as unknown as { __marker?: string }).__marker ?? null);

describe('hmr-dynamic-entry-cache', () => {
  test('keeps dynamic entry references synchronized across reload rebuilds', async () => {
    await waitForBuildStable();
    await expect.poll(() => page.textContent('.value')).toBe('bundle ok');

    await plantMarker();
    editFile('main.ts', (code) => code.replace("document.body.dataset.loaded = 'true';", ''));
    await expect.poll(readMarker).toBe(null);
    await expect.poll(() => page.textContent('.value')).toBe('bundle ok');
    await waitForBuildStable();

    await plantMarker();
    editFile('main.ts', (code) =>
      code.replace(
        /\(async \(\) => \{[\s\S]*\}\)\(\);/,
        "document.querySelector('.value')!.textContent = 'dynamic import removed';",
      ),
    );
    await expect.poll(readMarker).toBe(null);
    await expect.poll(() => page.textContent('.value')).toBe('dynamic import removed');
    await waitForBuildStable();
  });
});
