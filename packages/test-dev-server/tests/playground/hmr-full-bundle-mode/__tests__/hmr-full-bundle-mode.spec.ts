import { setTimeout } from 'node:timers/promises';
import { describe, expect, test } from 'vitest';
import { editFile, page, serverLogs, waitForBuildStable } from '~utils';

// This playground's scenarios all exercise ONE shared full-bundle (index.html
// renders every div, main.js imports every sub-module), so they must run on one
// page and one server — they live here as `describe` blocks that run in order,
// each restoring the files it edits. (A first-build failure can't be tested on
// this already-built server, so it lives in its own `initial-build-error`
// playground instead.)

const SLOT = '/* @syntax-error-slot */';
const BREAK = "const broken = '";

describe('hmr-full-bundle-mode', () => {
  test('should render initial content', async () => {
    const headingText = await page.textContent('h1');
    expect(headingText).toBe('HMR Full Bundle Mode');

    const appText = await page.textContent('.app');
    expect(appText).toBe('hello');

    const hmrText = await page.textContent('.hmr');
    expect(hmrText).toBe('hello');
  });

  test('basic HMR', async () => {
    editFile('hmr.js', (code) => code.replace("const foo = 'hello'", "const foo = 'hello1'"));

    await expect.poll(() => page.textContent('.hmr')).toBe('hello1');

    // Wait for the build to settle so the watcher sees the next edit as a
    // new change.
    await waitForBuildStable();
    editFile('hmr.js', (code) => code.replace("const foo = 'hello1'", "const foo = 'hello2'"));

    await expect.poll(() => page.textContent('.hmr')).toBe('hello2');

    await waitForBuildStable();
    editFile('hmr.js', (code) => code.replace("const foo = 'hello2'", "const foo = 'hello'"));
    await expect.poll(() => page.textContent('.hmr')).toBe('hello');
  });

  // https://github.com/vitejs/rolldown-vite/blob/942cb2b51b59fd6aefe886ec78eb34fff56ead34/playground/hmr-full-bundle-mode/__tests__/hmr-full-bundle-mode.spec.ts#L49-L70
  test('debounce bundle', async () => {
    editFile('main.js', (code) =>
      code.replace("text('.app', 'hello')", "text('.app', 'hello1')\n" + '// @delay-transform'),
    );
    await setTimeout(100);
    editFile('main.js', (code) => code.replace("text('.app', 'hello1')", "text('.app', 'hello2')"));
    await expect.poll(() => page.textContent('.app')).toBe('hello2');

    editFile('main.js', (code) =>
      code.replace("text('.app', 'hello2')\n" + '// @delay-transform', "text('.app', 'hello')"),
    );
    await expect.poll(() => page.textContent('.app')).toBe('hello');
  });

  // https://github.com/vitejs/rolldown-vite/blob/942cb2b51b59fd6aefe886ec78eb34fff56ead34/playground/hmr-full-bundle-mode/__tests__/hmr-full-bundle-mode.spec.ts#L101-L123
  test('continuous generate hmr patch', async () => {
    editFile('hmr.js', (code) =>
      code.replace("const foo = 'hello'", "const foo = 'hello1'\n" + '// @delay-transform'),
    );
    await setTimeout(100);
    editFile('hmr.js', (code) => code.replace("const foo = 'hello1'", "const foo = 'hello2'"));
    await expect.poll(() => page.textContent('.hmr')).toBe('hello2');

    editFile('hmr.js', (code) =>
      code.replace("const foo = 'hello2'\n" + '// @delay-transform', "const foo = 'hello'"),
    );
    await expect.poll(() => page.textContent('.hmr')).toBe('hello');
  });

  // The dev server injects its own error overlay (`#rolldown-error-overlay`)
  // into the served HTML. It should appear when the build breaks and clear
  // when the file is fixed.
  test('shows build-error overlay and recovers on fix', async () => {
    await waitForBuildStable();

    // Break the file with a syntax error (unterminated string).
    editFile('hmr.js', (code) => code.replace("const foo = 'hello'", "const foo = 'hello"));

    const overlay = page.locator('#rolldown-error-overlay');
    await expect.poll(() => overlay.count(), { timeout: 15_000 }).toBe(1);
    expect(await overlay.textContent()).toMatch(/Unterminated|PARSE_ERROR|error/i);

    // Fix it: the overlay clears and the app renders again.
    editFile('hmr.js', (code) => code.replace("const foo = 'hello", "const foo = 'hello'"));
    await expect.poll(() => overlay.count(), { timeout: 15_000 }).toBe(0);
    await expect.poll(() => page.textContent('.hmr')).toBe('hello');

    await waitForBuildStable();
  });
});

// Covers the design principles in meta/design/dev-engine.md for an HMR
// failure: a syntax error makes the HMR update fail and the overlay shows
// (Design Principle 2). Refreshing the page then triggers a full rebuild —
// the one exception in Design Principle 3 where page access starts a build,
// to get past a possibly broken HMR path. Here the source is still broken,
// so that build fails too; after it, refreshing triggers nothing (Design
// Principle 1). Fixing the file recovers (Design Principle 3).
describe('hmr-full-bundle-mode: HMR-stage failure', () => {
  test('page refresh after an Hmr-stage failure triggers a full rebuild', async () => {
    await waitForBuildStable();

    // Break the file with a syntax error; the HMR update fails.
    editFile('hmr-error/module.js', (code) => code.replace(SLOT, BREAK));

    const overlay = page.locator('#rolldown-error-overlay');
    await expect.poll(() => overlay.count(), { timeout: 15_000 }).toBe(1);
    // The page still runs the last good bundle.
    expect(await page.textContent('.hmr-error')).toBe('hmr-error: ok');

    const { buildSeq: seqWhileBroken, lastBuildErrored } = await waitForBuildStable();
    expect(lastBuildErrored).toBe(true);

    // The exception in Design Principle 3: reload after an HMR failure
    // triggers a full rebuild. It fails again (the file is still broken),
    // but a new build ran — buildSeq moved. Compare rebuild-stage failure,
    // where a reload builds nothing.
    await page.reload();
    const afterReload = await waitForBuildStable();
    expect(afterReload.buildSeq).toBeGreaterThan(seqWhileBroken);
    expect(afterReload.lastBuildErrored).toBe(true);
    await expect.poll(() => overlay.count(), { timeout: 15_000 }).toBe(1);

    // The failure is now a full-build failure, not an HMR one — so another
    // reload triggers nothing (Design Principle 1).
    await page.reload();
    await expect.poll(() => overlay.count(), { timeout: 15_000 }).toBe(1);
    const afterSecondReload = await waitForBuildStable();
    expect(afterSecondReload.buildSeq).toBe(afterReload.buildSeq);

    // Design Principle 3: fix the file — the build succeeds, the server
    // reloads the page, and the overlay clears.
    editFile('hmr-error/module.js', (code) => code.replace(BREAK, SLOT));
    await expect
      .poll(() => page.textContent('.hmr-error'), { timeout: 15_000 })
      .toBe('hmr-error: ok');
    await expect.poll(() => overlay.count()).toBe(0);
    await waitForBuildStable();
  });
});

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
    await expect
      .poll(() => page.textContent('.rebuild-error'), { timeout: 15_000 })
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
