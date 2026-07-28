import { describe, expect, test } from 'vitest';
import {
  editFile,
  errorOverlayText,
  page,
  plantReloadMarker,
  readFile,
  readReloadMarker,
  removeFile,
  waitForBuildStable,
} from '~utils';

// End-to-end check of `hotUpdate` for real watcher DELETE events (see
// dev.config.mjs). `main.js` self-accepts and counts its own runs; the hook
// logs every delete it sees to `hook-log.txt`, which is the positive proof
// that a noop round actually went through the hook instead of the event
// getting lost.

const readMainRuns = () =>
  page.evaluate(() => (window as unknown as { __mainRuns?: number }).__mainRuns ?? -1);

const readHookLog = () => {
  try {
    return readFile('hook-log.txt');
  } catch {
    return '';
  }
};

describe('hmr-hot-update-hook-delete', () => {
  test('renders the initial value', async () => {
    await waitForBuildStable();
    await expect.poll(() => page.textContent('.value')).toBe('runs:1');
  });

  test('drop the child imports so the deletes hit orphaned modules', async () => {
    await waitForBuildStable();
    await plantReloadMarker();

    editFile('main.js', (code) =>
      code.replace("import './child-a.js';\n", '').replace("import './child-b.js';\n", ''));
    // main self-accepts, so its edit re-runs main in place.
    await expect.poll(readMainRuns).toBe(2);

    expect(await readReloadMarker()).toBe('alive');
    await waitForBuildStable();
  });

  test('delete + decline: the hook sees the deleted module, default flow noops', async () => {
    await waitForBuildStable();
    await plantReloadMarker();

    removeFile('child-a.js');
    // The hook saw the delete with the deleted module itself in the default
    // set — deletion does not pre-empt the chain.
    await expect.poll(readHookLog).toContain('delete child-a.js ["child-a.js"]');

    // The hook declined and child-a has no importers anymore, so the round
    // must end as a noop: no re-run, no reload, no error.
    await waitForBuildStable();
    expect(await readMainRuns()).toBe(2);
    expect(await readReloadMarker()).toBe('alive');
    expect(await errorOverlayText()).toBe('');
  });

  test('delete + replace: the returned module ships even though its code is unchanged', async () => {
    await waitForBuildStable();
    await plantReloadMarker();

    removeFile('child-b.js');
    await expect.poll(readHookLog).toContain('delete child-b.js ["child-b.js"]');

    // The hook returned [main.js]: main re-runs in place although its own
    // code did not change (hook-returned modules skip the unchanged-output
    // suppression), and no importer expansion is applied to it.
    await expect.poll(readMainRuns).toBe(3);
    expect(await readReloadMarker()).toBe('alive');
    expect(await errorOverlayText()).toBe('');
    await waitForBuildStable();
  });
});
