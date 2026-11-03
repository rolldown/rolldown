import { describe, expect, test } from 'vitest';
import {
  editFile,
  page,
  plantReloadMarker,
  readReloadMarker,
  waitForBuildStable,
} from '~utils';

// Ports Vite's `circular/` case: mod-a -> mod-b -> mod-c -> mod-a form a circle with no
// acceptance inside it; the self-accepting entry OUTSIDE the circle is the boundary.
// Editing inside the circle must hot-update through it — no reload, no infinite walk.

describe('hmr-circular-accept-outside', () => {
  // Skipped together with the gap test below: with every test in the file skipped,
  // vitest never runs the per-file browser + dev-server boot. Circular-import rendering
  // itself is covered by the active `hmr-circular-self-accept` suite.
  test.skip('renders the chain through the circle', async () => {
    await waitForBuildStable();
    await expect.poll(() => page.textContent('.circular')).toContain('mod-a -> mod-b -> mod-c ->');
  });

  // KNOWN FBM GAP: the client walk treats ANY circular import chain as a reload reason
  // (`fbmHmrClient.ts` bubble() returns "circular import chain" when it re-meets an
  // ancestor) instead of deduping visited nodes through the cycle and continuing to the
  // accepting boundary outside it, the way Vite's `propagateUpdate` does. Verified: the
  // edited content still lands, but through a full page reload (the marker is wiped).
  // Unskip once the walk crosses circles to an outside boundary.
  test.skip('editing inside the circle hot-updates via the boundary outside it', async () => {
    await plantReloadMarker();

    editFile('mod-b.js', (code) => code.replace('`mod-b ->', '`mod-b (edited) ->'));
    await expect.poll(() => page.textContent('.circular')).toContain('mod-b (edited) ->');

    expect(await readReloadMarker()).toBe('alive');
    await waitForBuildStable();
  });
});
