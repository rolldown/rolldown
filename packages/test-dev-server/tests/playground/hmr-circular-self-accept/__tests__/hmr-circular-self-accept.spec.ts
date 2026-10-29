import { describe, expect, test } from 'vitest';
import { editFile, page, waitForBuildStable } from '~utils';

// Ports Vite's #2255 case: a module that self-accepts while sitting inside a circular
// import group (c imports b, b imports c) must still receive its hot update. Vite only
// asserts the new content lands (its own client may error and refresh); this asserts the
// same end state.

describe('hmr-circular-self-accept', () => {
  test('renders the value from inside the circle', async () => {
    await waitForBuildStable();
    await expect.poll(() => page.textContent('.self-accept-within-circular')).toBe('c');
  });

  test('self-accepted module within the circle receives the update', async () => {
    // No leading waitForBuildStable: the previous test already proved stability and
    // nothing has been edited since; every call costs its full stability window.
    editFile('c.js', (code) => code.replace("export const c = 'c'", "export const c = 'cc'"));
    await expect.poll(() => page.textContent('.self-accept-within-circular')).toBe('cc');
    await waitForBuildStable();
  });
});
