import { setTimeout } from 'node:timers/promises';
import { describe, expect, test } from 'vitest';
import { CONFIG } from './src/config';
import { editFile, getLazyPage, getPage, waitForBuildStable } from './test-utils';

const LAZY_URL = `http://localhost:${CONFIG.ports.lazyCompilation}`;

describe('hmr-full-bundle-mode', () => {
  test.sequential('should render initial content', async () => {
    const page = getPage();

    const headingText = await page.textContent('h1');
    expect(headingText).toBe('HMR Full Bundle Mode');

    const appText = await page.textContent('.app');
    expect(appText).toBe('hello');

    const hmrText = await page.textContent('.hmr');
    expect(hmrText).toBe('hello');
  });

  test.sequential('basic HMR', async () => {
    const page = getPage();

    await editFile('hmr.js', (code) => code.replace("const foo = 'hello'", "const foo = 'hello1'"));

    await expect.poll(() => page.textContent('.hmr')).toBe('hello1');

    // Wait for the build to stabilize before the next edit so the watcher's
    // debounce window has closed and will detect the new change.
    await waitForBuildStable(CONFIG.ports.hmrFullBundleMode);
    await editFile('hmr.js', (code) =>
      code.replace("const foo = 'hello1'", "const foo = 'hello2'"),
    );

    await expect.poll(() => page.textContent('.hmr')).toBe('hello2');

    await waitForBuildStable(CONFIG.ports.hmrFullBundleMode);
    await editFile('hmr.js', (code) => code.replace("const foo = 'hello2'", "const foo = 'hello'"));
    await expect.poll(() => page.textContent('.hmr')).toBe('hello');
  });

  // https://github.com/vitejs/rolldown-vite/blob/942cb2b51b59fd6aefe886ec78eb34fff56ead34/playground/hmr-full-bundle-mode/__tests__/hmr-full-bundle-mode.spec.ts#L49-L70
  test.sequential('debounce bundle', async () => {
    const page = getPage();
    editFile('main.js', (code) =>
      code.replace("text('.app', 'hello')", "text('.app', 'hello1')\n" + '// @delay-transform'),
    );
    await setTimeout(100);
    await editFile('main.js', (code) =>
      code.replace("text('.app', 'hello1')", "text('.app', 'hello2')"),
    );
    await expect.poll(() => page.textContent('.app')).toBe('hello2');

    await editFile('main.js', (code) =>
      code.replace("text('.app', 'hello2')\n" + '// @delay-transform', "text('.app', 'hello')"),
    );
    await expect.poll(() => page.textContent('.app')).toBe('hello');
  });

  // https://github.com/vitejs/rolldown-vite/blob/942cb2b51b59fd6aefe886ec78eb34fff56ead34/playground/hmr-full-bundle-mode/__tests__/hmr-full-bundle-mode.spec.ts#L101-L123
  test.sequential('continuous generate hmr patch', async () => {
    const page = getPage();

    await editFile('hmr.js', (code) =>
      code.replace("const foo = 'hello'", "const foo = 'hello1'\n" + '// @delay-transform'),
    );
    await setTimeout(100);
    editFile('hmr.js', (code) => code.replace("const foo = 'hello1'", "const foo = 'hello2'"));
    await expect.poll(() => page.textContent('.hmr')).toBe('hello2');

    await editFile('hmr.js', (code) =>
      code.replace("const foo = 'hello2'\n" + '// @delay-transform', "const foo = 'hello'"),
    );
    await expect.poll(() => page.textContent('.hmr')).toBe('hello');
  });
});

describe('lazy-compilation', () => {
  test.sequential('should load lazy module on demand', async () => {
    const page = getLazyPage();

    // Track JS requests to verify lazy loading pattern.
    // The page is navigated here (not in beforeAll) so the lazy-compilation
    // server sees a cold first request — no prior warming from setup.
    const jsRequests: string[] = [];
    page.on('request', (req: { url: () => string }) => {
      const url = req.url();
      if (url.includes('.js')) {
        jsRequests.push(url);
      }
    });

    await page.goto(LAZY_URL, { waitUntil: 'domcontentloaded' });

    // 1. Verify main module loaded
    await expect.poll(() => page.textContent('.status')).toBe('main loaded');

    // 2. Wait for lazy module to load (triggered by setTimeout in main.js)
    await expect.poll(() => page.textContent('.lazy-result')).toBe('lazy-loaded');

    // 3. Verify lazy compilation produced separate chunks:
    // - One for main.js
    // - Two for lazy-module (proxy chunk + actual chunk with different hashes)
    // This would NOT happen with eager bundling where everything is in one bundle.
    const lazyModuleChunks = jsRequests.filter((url) => url.includes('lazy-module'));
    expect(lazyModuleChunks.length).toBe(2);
  });
});
