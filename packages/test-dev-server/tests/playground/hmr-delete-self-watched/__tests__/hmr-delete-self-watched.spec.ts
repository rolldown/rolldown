import { describe, expect, test } from 'vitest';
import {
  addFile,
  errorOverlayText,
  page,
  removeFile,
  waitForBuildStable,
} from '~utils';

// child.js registers ITSELF with `addWatchFile` (see dev.config.mjs), so a
// delete event reaches the engine through two routes: the own-module lookup
// and the transform-dependency lookup.
//
// Deleting a file that is still imported must FAIL the round: the rebuild
// re-resolves main.js's imports against the real filesystem, so the missing
// file surfaces as an unresolved-import error in Vite's overlay. The pin here
// is that the self-watch route adds no second failure mode on top of that —
// the server must survive the round and recover cleanly when the file comes
// back, instead of crashing or wedging on a re-read of the deleted module.

describe('hmr-delete-self-watched', () => {
  test('renders the initial value', async () => {
    await waitForBuildStable();
    await expect.poll(() => page.textContent('.value')).toBe('child-v1');
  });

  test('deleting the self-watched module surfaces a resolve error', async () => {
    await waitForBuildStable();

    removeFile('child.js');
    await expect.poll(errorOverlayText).toMatch(/Could not resolve '\.\/child\.js'/);
    await waitForBuildStable();
  });

  test('recreating the file with changed content recovers', async () => {
    await waitForBuildStable();

    addFile('child.js', "export const childValue = 'child-v2';\n");
    // Recovery may arrive as a patch or as a full reload — assert the outcome:
    // the new value renders and the overlay is gone.
    await expect.poll(() => page.textContent('.value')).toBe('child-v2');
    await expect.poll(errorOverlayText).toBe('');
    await waitForBuildStable();
  });
});
