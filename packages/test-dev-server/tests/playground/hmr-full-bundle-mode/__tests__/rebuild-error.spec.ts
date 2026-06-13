import { describe, expect, test } from 'vitest';
import { editFile, getBuildSeq, page, readFile, serverLogs, waitForBuildStable } from '~utils';

// Covers the design principles in meta/design/dev-engine.md for a rebuild
// failure:
// - Design Principle 1 (Conservative rebuilds): opening or refreshing the
//   page never starts or retries a build
// - Design Principle 2 (Errors are emitted on every build): every failed
//   build reports its own error, also to clients that reconnect
// - Design Principle 3 (File changes are the only recovery trigger): only a
//   file change recovers
describe('hmr-full-bundle-mode: rebuild-stage failure', () => {
  test('page access on fresh output does not rebuild', async () => {
    // Design Principle 1: page access never triggers a build.
    const { buildSeq: seqFresh } = await waitForBuildStable();

    await page.reload();
    await expect.poll(() => page.textContent('.rebuild-error')).toBe('rebuild-error: ok');

    const status = await waitForBuildStable();
    expect(status.buildSeq).toBe(seqFresh);
    expect(status.hasStaleOutput).toBe(false);
    expect(status.lastBuildErrored).toBe(false);
  });

  test('refresh never retries a failed rebuild; a file change recovers it', async () => {
    await waitForBuildStable();

    // Arm the failure. The flag file is not watched, so editing it does not
    // trigger a build by itself.
    editFile('rebuild-error/flag.txt', () => 'broken-1');

    // This module is not self-accepting, so editing it forces a rebuild —
    // and generateBundle now throws.
    editFile('rebuild-error/module.js', (code) =>
      code.replace("'rebuild-error: ok'", "'rebuild-error: updated'"),
    );

    const overlay = page.locator('#rolldown-error-overlay');
    await expect.poll(() => overlay.count(), { timeout: 15_000 }).toBe(1);
    expect(await overlay.textContent()).toContain('generateBundle broken by flag: broken-1');
    // The failed rebuild did not reload the page; it still runs the old bundle.
    expect(await page.textContent('.rebuild-error')).toBe('rebuild-error: ok');
    // Build errors also reach the terminal.
    expect(serverLogs.some((log) => log.includes('Build error'))).toBe(true);

    // Design Principle 1: refreshing must not retry the build — without new
    // input the same error would just happen again. The new client gets the
    // saved error instead (Design Principle 2).
    const { buildSeq: seqFailed, lastBuildErrored } = await waitForBuildStable();
    expect(lastBuildErrored).toBe(true);
    await page.reload();
    await expect.poll(() => overlay.count(), { timeout: 15_000 }).toBe(1);
    const afterReload = await waitForBuildStable();
    expect(afterReload.buildSeq).toBe(seqFailed);
    expect(afterReload.lastBuildErrored).toBe(true);

    // Design Principle 2: each failed build reports its own error — another
    // change while still broken shows the new message, not the old one.
    editFile('rebuild-error/flag.txt', () => 'broken-2');
    editFile('rebuild-error/module.js', (code) =>
      code.replace("'rebuild-error: updated'", "'rebuild-error: updated-2'"),
    );
    await expect
      .poll(() => overlay.textContent({ timeout: 500 }).catch(() => ''), { timeout: 15_000 })
      .toContain('generateBundle broken by flag: broken-2');

    // Design Principle 3: a file change recovers. Disarm the flag, then
    // touch the module — the build succeeds and the server reloads the page
    // onto the fresh bundle.
    editFile('rebuild-error/flag.txt', () => 'ok');
    editFile('rebuild-error/module.js', (code) =>
      code.replace("'rebuild-error: updated-2'", "'rebuild-error: recovered'"),
    );
    // DEBUG (CI flake #9727): on the constrained CI runner this recovery poll
    // sometimes never sees 'recovered'. Poll up to 30s (well past the normal
    // 15s) to learn whether it recovers late or is truly stuck, and on each
    // tick print what `.rebuild-error` reads, the overlay count, and any new
    // server-log lines — overlay gone means the rebuild recovered server-side,
    // so a value stuck at 'ok' means the page never reloaded onto the fresh
    // bundle. Grep the log for `[recover]`.
    const recoverStart = Date.now();
    let serverLogsSeen = serverLogs.length;
    await expect
      .poll(
        async () => {
          const text = await page.textContent('.rebuild-error');
          const overlayCount = await overlay.count();
          const elapsed = Date.now() - recoverStart;
          // Read what is actually on disk this tick — confirms the recovery
          // write landed (and stays) even while the server never reacts.
          const diskModule =
            readFile('rebuild-error/module.js').match(/value = (['"].*?['"])/)?.[1] ?? '<no value>';
          const diskFlag = readFile('rebuild-error/flag.txt');
          // buildSeq from /_dev/status: if it never increments, the recovery
          // edit never triggered a build (a dropped file-watch event).
          const buildSeq = await getBuildSeq().catch(() => -1);
          for (const log of serverLogs.slice(serverLogsSeen)) {
            console.log(`[recover] +${elapsed}ms serverLog: ${log}`);
          }
          serverLogsSeen = serverLogs.length;
          console.log(
            `[recover] +${elapsed}ms .rebuild-error=${JSON.stringify(text)} overlay=${overlayCount} buildSeq=${buildSeq} disk:module.value=${diskModule} disk:flag=${JSON.stringify(diskFlag)}`,
          );
          return text;
        },
        { timeout: 30_000, interval: 250 },
      )
      .toBe('rebuild-error: recovered');
    await expect.poll(() => overlay.count()).toBe(0);

    // Restore the fixture.
    await waitForBuildStable();
    editFile('rebuild-error/module.js', (code) =>
      code.replace("'rebuild-error: recovered'", "'rebuild-error: ok'"),
    );
    await expect
      .poll(() => page.textContent('.rebuild-error'), { timeout: 15_000 })
      .toBe('rebuild-error: ok');
    await waitForBuildStable();
  });
});
