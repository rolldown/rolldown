import { setTimeout } from 'node:timers/promises';
import { describe, expect, test } from 'vitest';
import { CONFIG } from './src/config';
import {
  editFile,
  editLazySharedModuleFile,
  getLazyPage,
  getLazySharedModulePage,
  getNestedLazyPage,
  getPage,
  waitForBuildStable,
} from './test-utils';

const LAZY_URL = `http://localhost:${CONFIG.ports.lazyCompilation}`;
const LAZY_SHARED_MODULE_URL = `http://localhost:${CONFIG.ports.lazySharedModule}`;
const NESTED_LAZY_URL = `http://localhost:${CONFIG.ports.lazyNestedDynamicImport}`;

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

describe('lazy-shared-module', () => {
  // Regression test for PR #9132 (issue #9312): when a dynamically imported
  // module is shared by multiple lazy entries (so it lands in a
  // `ChunkKind::Common` chunk with minified export keys), the fetched proxy
  // must read exports from the runtime registry via `loadExports` rather than
  // returning the raw chunk namespace. Before the fix, `sel.foo` was
  // `undefined` because the namespace key was an alias like `$` instead of
  // `foo`.
  test.sequential(
    'preserves original export names for shared lazy module (#9312)',
    { retry: 0 },
    async () => {
      const page = getLazySharedModulePage();

      // First load: triggers initial lazy fetches through the NOT-fetched proxy
      // template. The server marks each proxy fetched and rebuilds main.js to
      // embed the *fetched* proxy chunks. We don't assert against this load — we
      // need the rebuilt output to land first, because the bug only manifests
      // through the fetched-template path.
      await page.goto(LAZY_SHARED_MODULE_URL, { waitUntil: 'domcontentloaded' });
      await page.click('#btn');
      await expect.poll(() => page.textContent('#status')).toBe('done');

      // Wait for the rebuild(s) triggered by mark_as_fetched to settle.
      await waitForBuildStable(CONFIG.ports.lazySharedModule);

      // Second load: main.js now imports the fetched-proxy chunks for page-a,
      // page-b, and selectors. This is the path PR #9132 fixes — before the fix,
      // returning the raw chunk namespace meant `sel.foo === undefined` because
      // chunk-level export aliasing renamed `foo`/`bar` to short identifiers.
      await page.reload({ waitUntil: 'domcontentloaded' });
      await page.click('#btn');
      await expect.poll(() => page.textContent('#status')).toBe('done');

      const log = (await page.textContent('#log')) ?? '';
      expect(log).toContain('page-a.a = foo_value');
      expect(log).toContain('page-b.b = bar_value');
      expect(log).toContain('sel.foo = foo_value');
      expect(log).toContain('sel.bar = bar_value');
      expect(log).not.toContain('UNDEFINED');
    },
  );

  // `selectors.js` is pulled into the build only via lazy compilation (the
  // initial graph contains just `app.js`; `page-a`, `page-b`, and `selectors`
  // arrive through the dynamic-import proxies). The dev coordinator must
  // register `selectors.js` with the FS watcher after `compile_lazy_entry`,
  // otherwise edits to it are invisible and no full reload is dispatched.
  // This test asserts the auto-reload path end-to-end: edit the file, let the
  // dev server reload the page on its own, and verify the new values.
  test.sequential('should watch and auto-reload a lazy-loaded module', { retry: 0 }, async () => {
    const page = getLazySharedModulePage();

    await editLazySharedModuleFile('selectors.js', (code) =>
      code
        .replace("'foo_value'", "'foo_value_updated'")
        .replace("'bar_value'", "'bar_value_updated'"),
    );

    // The previous test left `#status` as 'done'. A successful full reload
    // resets it to the HTML default 'pending'.
    await expect.poll(() => page.textContent('#status'), { timeout: 15_000 }).toBe('pending');

    await waitForBuildStable(CONFIG.ports.lazySharedModule);

    await page.click('#btn');
    await expect.poll(() => page.textContent('#status')).toBe('done');

    const log = (await page.textContent('#log')) ?? '';
    expect(log).toContain('page-a.a = foo_value_updated');
    expect(log).toContain('page-b.b = bar_value_updated');
    expect(log).toContain('sel.foo = foo_value_updated');
    expect(log).toContain('sel.bar = bar_value_updated');
    expect(log).not.toContain('UNDEFINED');
  });
});

// Regression test for the fix in
// `crates/rolldown/src/hmr/hmr_ast_finalizer.rs::try_rewrite_dynamic_import`
// (the `?rolldown-lazy=1` branch). `outer.js` is itself loaded as a lazy chunk,
// and its body contains `await import('./inner.js')`, which also resolves to a
// lazy proxy. Before the fix the HMR AST finalizer rewrote that inner dynamic
// import to a plain `import('/@vite/lazy?id=...')`, returning the partial
// bundle's raw namespace — which has no `'rolldown:exports'` key, so
// `__unwrap_lazy_compilation_entry` fell through and `inner.foo` came back
// `undefined`. The fix chains `.then(() => loadExports("<stable_proxy_id>"))`
// so the namespace read by the unwrap helper is the proxy module's registered
// exports (which carry the `'rolldown:exports'` getter).
describe('lazy-nested-dynamic-import', () => {
  // `retry: 0` is critical: the bug only manifests on the first click of a fresh
  // page. The first click triggers `mark_as_fetched`, which rebuilds main.js so
  // it references the *fetched* proxy chunk — and the fetched proxy chunk imports
  // the full-build outer.js (compiled by scope_finalizer), bypassing the buggy
  // HMR-finalizer rewrite entirely. With retries enabled, the reload between
  // attempts would always land on the post-rebuild fetched-proxy path and mask
  // the regression.
  test.sequential(
    'lazy chunk can dynamically import another lazy chunk',
    { retry: 0 },
    async () => {
      const page = getNestedLazyPage();

      await page.goto(NESTED_LAZY_URL, { waitUntil: 'domcontentloaded' });
      await page.click('#btn');
      await expect.poll(() => page.textContent('#status')).toBe('done');

      const log = (await page.textContent('#log')) ?? '';
      expect(log).toContain('outer.outerName = outer');
      expect(log).toContain('inner.foo = inner_foo');
      expect(log).toContain('inner.bar = inner_bar');
      expect(log).not.toContain('UNDEFINED');
    },
  );
});
