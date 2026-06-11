import { setTimeout } from 'node:timers/promises';
import { describe, expect, test } from 'vitest';
import { editFile, page, waitForBuildStable } from '~utils';

// Tests run sequentially within a file (the default) and share one page and one
// in-process dev server. Each test restores the fixture it edits, so order and
// retries stay safe.
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

    // Wait for the build to stabilize before the next edit so the watcher's
    // debounce window has closed and will detect the new change.
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

  // The dev server injects its own build-error overlay (`#rolldown-error-overlay`)
  // into the served HTML — the shared rolldown HMR runtime is deliberately not
  // touched. This asserts the overlay appears on a build break and clears on
  // recovery (exercising the dev engine's error-recovery path end-to-end).
  test('shows build-error overlay and recovers on fix', async () => {
    await waitForBuildStable();

    // Introduce a syntax error (unterminated string).
    editFile('hmr.js', (code) => code.replace("const foo = 'hello'", "const foo = 'hello"));

    const overlay = page.locator('#rolldown-error-overlay');
    await expect.poll(() => overlay.count(), { timeout: 15_000 }).toBe(1);
    expect(await overlay.textContent()).toMatch(/Unterminated|PARSE_ERROR|error/i);

    // Fix it: overlay clears (via recovery reload) and the app renders again.
    editFile('hmr.js', (code) => code.replace("const foo = 'hello", "const foo = 'hello'"));
    await expect.poll(() => overlay.count(), { timeout: 15_000 }).toBe(0);
    await expect.poll(() => page.textContent('.hmr')).toBe('hello');

    await waitForBuildStable();
  });
});
