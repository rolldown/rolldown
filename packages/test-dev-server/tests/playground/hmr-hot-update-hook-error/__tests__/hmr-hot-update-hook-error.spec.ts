import { describe, expect, test } from 'vitest';
import { editFile, errorOverlayText, page, waitForBuildStable } from '~utils';

// End-to-end check of a throwing `hotUpdate` hook (see dev.config.mjs). The
// key pin is the middle test: the failed round's dep edit must NOT be
// retried by the next round — a hook error queues nothing in
// `pending_rescans`, so dep's new content reaches the browser only when
// dep.js itself changes again.

describe('hmr-hot-update-hook-error', () => {
  test('renders the initial values', async () => {
    await waitForBuildStable();
    await expect.poll(() => page.textContent('.dep')).toBe('dep-v1');
    await expect.poll(() => page.textContent('.other')).toBe('other-m1');
  });

  test('a hook error fails the round and surfaces in the overlay', async () => {
    await waitForBuildStable();

    editFile('dep.js', (code) => code.replace('dep-v1', 'dep-v2'));
    await expect.poll(errorOverlayText).toContain('hotUpdate hook failed on purpose');

    // The round died before the edit reached the graph.
    expect(await page.textContent('.dep')).toBe('dep-v1');
    await waitForBuildStable();
  });

  test('the failed edit is not retried: an unrelated edit ships alone', async () => {
    await waitForBuildStable();

    editFile('other.js', (code) => code.replace('other-m1', 'other-m2'));
    // Recovery from the errored round may arrive as a patch or as a full
    // reload — assert the outcome. Either way dep's lost v2 edit was not
    // queued anywhere: even a reload rebuilds from a graph that never saw it,
    // so the browser still runs v1.
    await expect.poll(() => page.textContent('.other')).toBe('other-m2');
    expect(await page.textContent('.dep')).toBe('dep-v1');
    await expect.poll(errorOverlayText).toBe('');
    await waitForBuildStable();
  });

  test('editing the file again delivers its latest content and clears the error', async () => {
    await waitForBuildStable();

    editFile('dep.js', (code) => code.replace('dep-v2', 'dep-v3'));
    await expect.poll(() => page.textContent('.dep')).toBe('dep-v3');
    await expect.poll(errorOverlayText).toBe('');
    await waitForBuildStable();
  });
});
