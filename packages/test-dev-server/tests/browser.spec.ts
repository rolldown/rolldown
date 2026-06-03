import { setTimeout } from 'node:timers/promises';
import type { Page, Request } from 'playwright';
import { describe, expect, test } from 'vitest';
import { CONFIG } from './src/config';
import {
  editFile,
  editLazySharedModuleFile,
  getClientRegisteredModules,
  getLazyAliasedImportPage,
  getLazyPage,
  getLazySharedModulePage,
  getNestedLazyPage,
  getPage,
  waitForBuildStable,
  waitForClientModuleRegistration,
} from './test-utils';

const HMR_URL = `http://localhost:${CONFIG.ports.hmrFullBundleMode}`;
const LAZY_URL = `http://localhost:${CONFIG.ports.lazyCompilation}`;
const LAZY_SHARED_MODULE_URL = `http://localhost:${CONFIG.ports.lazySharedModule}`;
const NESTED_LAZY_URL = `http://localhost:${CONFIG.ports.lazyNestedDynamicImport}`;
const LAZY_ALIASED_IMPORT_URL = `http://localhost:${CONFIG.ports.lazyAliasedImport}`;

const IDENTITY_RENDER = 'export const render = (value) => value;';
const FACADE_RENDER = "import { decorate } from './facade.js';\nexport const render = decorate;";
const CJS_FACADE_RENDER =
  "import cjsFacade from './cjs-facade.cjs';\nexport const render = cjsFacade.decorate;";
const CONDITIONAL_CJS_FACADE_RENDER =
  "import conditionalCjsFacade from './conditional-cjs-facade.cjs';\n" +
  'export const render = conditionalCjsFacade.decorate;';
const CONDITIONAL_CHANGED_RENDER =
  "if (globalThis.__runConditionalChanged) require('./conditional-changed.cjs');\n" +
  IDENTITY_RENDER;

async function getHmrClientId(page: Page) {
  return page.evaluate(() => (globalThis as any).__rolldown_runtime__.clientId as string);
}

async function waitForHmrClientModules(page: Page, modules: string[]) {
  const clientId = await getHmrClientId(page);
  await waitForClientModuleRegistration(CONFIG.ports.hmrFullBundleMode, clientId, modules);
  return clientId;
}

async function openHmrPage(page: Page) {
  await page.goto(HMR_URL, { waitUntil: 'networkidle' });
  await expect.poll(() => page.textContent('.hmr')).toBe('hello');
  await waitForHmrClientModules(page, ['hmr.js']);
}

async function reloadHmrPage(page: Page) {
  await page.reload({ waitUntil: 'networkidle' });
  await expect.poll(() => page.textContent('.hmr')).toBe('hello');
  await waitForHmrClientModules(page, ['hmr.js']);
}

async function holdModuleRegistrationMessages(page: Page) {
  await page.evaluate(() => {
    const runtime = (globalThis as any).__rolldown_runtime__;
    const messenger = runtime.messenger;
    const originalSend = messenger.send;
    const heldMessages: unknown[] = [];

    messenger.send = (message: unknown) => {
      heldMessages.push(message);
    };
    (globalThis as any).__rolldownRegistrationHold = {
      heldMessages,
      release() {
        messenger.send = originalSend;
        for (const message of heldMessages) {
          originalSend.call(messenger, message);
        }
        delete (globalThis as any).__rolldownRegistrationHold;
      },
    };
  });

  await expect
    .poll(() =>
      page.evaluate(() => (globalThis as any).__rolldownRegistrationHold?.heldMessages.length ?? 0),
    )
    .toBe(0);

  return async () => {
    await page.evaluate(() => (globalThis as any).__rolldownRegistrationHold?.release());
  };
}

async function loadFacadeWithoutAcknowledging(page: Page) {
  const clientId = await getHmrClientId(page);
  const release = await holdModuleRegistrationMessages(page);

  try {
    await page.addScriptTag({ type: 'module', url: '/unloaded.js' });
    await expect
      .poll(() =>
        page.evaluate(
          () => (globalThis as any).__rolldownRegistrationHold?.heldMessages.length ?? 0,
        ),
      )
      .toBeGreaterThan(0);
    await expect
      .poll(() => page.evaluate(() => (globalThis as any).__implementationExecutions))
      .toBe(1);

    const registeredModules = await getClientRegisteredModules(
      CONFIG.ports.hmrFullBundleMode,
      clientId,
    );
    expect(registeredModules).not.toContain('facade.js');
    expect(registeredModules).not.toContain('implementation.js');

    return release;
  } catch (error) {
    await release().catch(() => {});
    throw error;
  }
}

function trackHmrPatchRequests(page: Page) {
  const urls: string[] = [];
  const onRequest = (request: Request) => {
    const url = new URL(request.url());
    if (/\/\d+\.js$/.test(url.pathname)) {
      urls.push(request.url());
    }
  };
  page.on('request', onRequest);
  return {
    urls,
    stop: () => page.off('request', onRequest),
  };
}

async function readHmrPatch(urls: string[]) {
  await expect.poll(() => urls.length).toBeGreaterThan(0);
  const response = await fetch(urls.at(-1)!);
  expect(response.ok).toBe(true);
  return response.text();
}

async function markPageInstance(page: Page) {
  const marker = `hmr-page-${Date.now()}-${Math.random()}`;
  await page.evaluate((value) => ((globalThis as any).__hmrPageInstance = value), marker);
  return async () => {
    await expect
      .poll(() => page.evaluate(() => (globalThis as any).__hmrPageInstance))
      .toBe(marker);
  };
}

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

  test.sequential('backfills missing static ESM dependencies', async () => {
    const page = getPage();
    const assertNoReload = await markPageInstance(page);

    try {
      await editFile('hmr.js', (code) => code.replace(IDENTITY_RENDER, FACADE_RENDER));
      await expect.poll(() => page.textContent('.hmr')).toBe('[hello]');
      await assertNoReload();
    } finally {
      await waitForBuildStable(CONFIG.ports.hmrFullBundleMode);
      await editFile('hmr.js', (code) => code.replace(FACADE_RENDER, IDENTITY_RENDER));
    }

    await expect.poll(() => page.textContent('.hmr')).toBe('hello');
    await waitForBuildStable(CONFIG.ports.hmrFullBundleMode);
  });

  test.sequential('backfills missing CommonJS require dependencies', async () => {
    const page = getPage();
    const assertNoReload = await markPageInstance(page);

    try {
      await editFile('hmr.js', (code) => code.replace(IDENTITY_RENDER, CJS_FACADE_RENDER));
      await expect.poll(() => page.textContent('.hmr')).toBe('[cjs:hello]');
      await assertNoReload();
    } finally {
      await waitForBuildStable(CONFIG.ports.hmrFullBundleMode);
      await editFile('hmr.js', (code) => code.replace(CJS_FACADE_RENDER, IDENTITY_RENDER));
    }

    await expect.poll(() => page.textContent('.hmr')).toBe('hello');
    await waitForBuildStable(CONFIG.ports.hmrFullBundleMode);
  });

  test.sequential('does not eagerly execute conditional CommonJS require dependencies', async () => {
    const page = getPage();
    const patches = trackHmrPatchRequests(page);
    const assertNoReload = await markPageInstance(page);

    try {
      await editFile('hmr.js', (code) =>
        code.replace(IDENTITY_RENDER, CONDITIONAL_CJS_FACADE_RENDER),
      );
      await expect.poll(() => page.textContent('.hmr')).toBe('[conditional:hello]');
      await expect
        .poll(() => page.evaluate(() => (globalThis as any).__conditionalCjsExecutions))
        .toBeUndefined();
      await assertNoReload();

      const patch = await readHmrPatch(patches.urls);
      expect(patch).toContain('conditional-cjs-facade.cjs');
      expect(patch).toContain('conditional-cjs-side-effect.cjs');
    } finally {
      patches.stop();
      await waitForBuildStable(CONFIG.ports.hmrFullBundleMode);
      await editFile('hmr.js', (code) =>
        code.replace(CONDITIONAL_CJS_FACADE_RENDER, IDENTITY_RENDER),
      );
    }

    await expect.poll(() => page.textContent('.hmr')).toBe('hello');
    await waitForBuildStable(CONFIG.ports.hmrFullBundleMode);
  });

  test.sequential('updates a loaded changed dependency behind a false conditional require', async () => {
    const loadedPage = getPage();
    const browser = loadedPage.context().browser();
    expect(browser).not.toBeNull();
    const unloadedContext = await browser!.newContext();
    const unloadedPage = await unloadedContext.newPage();

    try {
      await openHmrPage(unloadedPage);
      await loadedPage.evaluate(() => {
        (globalThis as any).__rolldown_runtime__.registerModule('conditional-changed.cjs', {
          exports: {},
        });
        (globalThis as any).__conditionalChangedValue = 'stale';
        (globalThis as any).__runConditionalChanged = false;
      });
      await waitForHmrClientModules(loadedPage, ['conditional-changed.cjs']);

      await editFile('hmr.js', (code) => code.replace(IDENTITY_RENDER, CONDITIONAL_CHANGED_RENDER));
      await waitForBuildStable(CONFIG.ports.hmrFullBundleMode);
      await editFile('conditional-changed.cjs', (code) => code.replace("'initial'", "'updated'"));

      await expect
        .poll(() => loadedPage.evaluate(() => (globalThis as any).__conditionalChangedValue))
        .toBe('updated');
      await expect
        .poll(() => unloadedPage.evaluate(() => (globalThis as any).__conditionalChangedValue))
        .toBeUndefined();
    } finally {
      await waitForBuildStable(CONFIG.ports.hmrFullBundleMode);
      await editFile('conditional-changed.cjs', (code) => code.replace("'updated'", "'initial'"));
      await waitForBuildStable(CONFIG.ports.hmrFullBundleMode);
      await editFile('hmr.js', (code) => code.replace(CONDITIONAL_CHANGED_RENDER, IDENTITY_RENDER));
      await unloadedContext.close();
    }

    await expect.poll(() => loadedPage.textContent('.hmr')).toBe('hello');
    await waitForBuildStable(CONFIG.ports.hmrFullBundleMode);
  });

  test.sequential('backfills an existing unloaded dynamic import target', async () => {
    const page = getPage();
    const assertNoReload = await markPageInstance(page);

    try {
      await page.addScriptTag({ type: 'module', url: '/dynamicImporter.js' });
      await page.evaluate(() => {
        (globalThis as any).__loadDynamicExisting = true;
      });
      await editFile('dynamic-importer.js', (code) => code.replace("'initial'", "'updated'"));
      await expect
        .poll(() => page.evaluate(() => (globalThis as any).__dynamicExistingValue))
        .toBe('dynamic-existing');
      await assertNoReload();
    } finally {
      await waitForBuildStable(CONFIG.ports.hmrFullBundleMode);
      await editFile('dynamic-importer.js', (code) => code.replace("'updated'", "'initial'"));
    }

    await waitForBuildStable(CONFIG.ports.hmrFullBundleMode);
  });

  test.sequential(
    'deduplicates a backfill before its real registration acknowledgement arrives',
    { retry: 0 },
    async () => {
      const page = getPage();
      const browser = page.context().browser();
      expect(browser).not.toBeNull();
      const context = await browser!.newContext();
      const racePage = await context.newPage();
      let release: (() => Promise<void>) | undefined;

      try {
        await openHmrPage(racePage);
        release = await loadFacadeWithoutAcknowledging(racePage);
        await editFile('hmr.js', (code) => code.replace(IDENTITY_RENDER, FACADE_RENDER));
        await expect.poll(() => racePage.textContent('.hmr')).toBe('[hello]');
        await expect
          .poll(() => racePage.evaluate(() => (globalThis as any).__implementationExecutions))
          .toBe(1);
      } finally {
        await release?.();
        await waitForBuildStable(CONFIG.ports.hmrFullBundleMode);
        await editFile('hmr.js', (code) => code.replace(FACADE_RENDER, IDENTITY_RENDER));
        await context.close();
      }

      await expect.poll(() => page.textContent('.hmr')).toBe('hello');
      await waitForBuildStable(CONFIG.ports.hmrFullBundleMode);
    },
  );

  test.sequential(
    'updates a self-accepting module before its registration acknowledgement arrives',
    { retry: 0 },
    async () => {
      const page = getPage();
      const browser = page.context().browser();
      expect(browser).not.toBeNull();
      const context = await browser!.newContext();
      const racePage = await context.newPage();
      await openHmrPage(racePage);
      const clientId = await getHmrClientId(racePage);
      const release = await holdModuleRegistrationMessages(racePage);

      await racePage.addScriptTag({ type: 'module', url: '/lateAccept.js' });
      await expect
        .poll(() => racePage.evaluate(() => (globalThis as any).__lateAcceptValue))
        .toBe('late');
      await expect
        .poll(() =>
          racePage.evaluate(
            () => (globalThis as any).__rolldownRegistrationHold?.heldMessages.length ?? 0,
          ),
        )
        .toBeGreaterThan(0);
      expect(
        await getClientRegisteredModules(CONFIG.ports.hmrFullBundleMode, clientId),
      ).not.toContain('late-accept.js');

      try {
        await editFile('late-accept.js', (code) =>
          code.replace("export const value = 'late';", "export const value = 'late2';"),
        );
        await expect
          .poll(() => racePage.evaluate(() => (globalThis as any).__lateAcceptValue))
          .toBe('late2');
        await expect
          .poll(() => page.evaluate(() => (globalThis as any).__lateAcceptValue))
          .toBeUndefined();
      } finally {
        await release();
        await waitForBuildStable(CONFIG.ports.hmrFullBundleMode);
        await editFile('late-accept.js', (code) =>
          code.replace("export const value = 'late2';", "export const value = 'late';"),
        );
        await context.close();
      }

      await waitForBuildStable(CONFIG.ports.hmrFullBundleMode);
    },
  );

  test.sequential(
    'reexecutes a backfilled ESM path when its dependency changed',
    { retry: 0 },
    async () => {
      const page = getPage();
      const browser = page.context().browser();
      expect(browser).not.toBeNull();
      const context = await browser!.newContext();
      const racePage = await context.newPage();
      let release: (() => Promise<void>) | undefined;

      const patches = trackHmrPatchRequests(racePage);
      try {
        await openHmrPage(racePage);
        release = await loadFacadeWithoutAcknowledging(racePage);
        await Promise.all([
          editFile('hmr.js', (code) => code.replace(IDENTITY_RENDER, FACADE_RENDER)),
          editFile('implementation.js', (code) =>
            code.replace('return `[${value}]`;', 'return `<${value}>`;'),
          ),
        ]);
        await expect.poll(() => racePage.textContent('.hmr')).toBe('<hello>');
        await expect
          .poll(() => racePage.evaluate(() => (globalThis as any).__implementationExecutions))
          .toBe(2);
        await waitForBuildStable(CONFIG.ports.hmrFullBundleMode);

        const patch = await readHmrPatch(patches.urls);
        expect(patch).toContain('facade.js');
        expect(patch).toContain('implementation.js');
      } finally {
        patches.stop();
        await release?.();
        await waitForBuildStable(CONFIG.ports.hmrFullBundleMode);
        await Promise.all([
          editFile('hmr.js', (code) => code.replace(FACADE_RENDER, IDENTITY_RENDER)),
          editFile('implementation.js', (code) =>
            code.replace('return `<${value}>`;', 'return `[${value}]`;'),
          ),
        ]);
        await context.close();
      }

      await expect.poll(() => page.textContent('.hmr')).toBe('hello');
      await waitForBuildStable(CONFIG.ports.hmrFullBundleMode);
    },
  );

  test.sequential(
    'renders different patches for clients with different registered modules',
    { retry: 0 },
    async () => {
      const loadedPage = getPage();
      await reloadHmrPage(loadedPage);

      const browser = loadedPage.context().browser();
      expect(browser).not.toBeNull();
      const missingContext = await browser!.newContext();
      const missingPage = await missingContext.newPage();
      try {
        await openHmrPage(missingPage);

        await loadedPage.addScriptTag({ type: 'module', url: '/unloaded.js' });
        await waitForHmrClientModules(loadedPage, ['facade.js', 'implementation.js']);

        const loadedPagePatches = trackHmrPatchRequests(loadedPage);
        const missingPagePatches = trackHmrPatchRequests(missingPage);
        try {
          await editFile('hmr.js', (code) => code.replace(IDENTITY_RENDER, FACADE_RENDER));
          await Promise.all([
            expect.poll(() => loadedPage.textContent('.hmr')).toBe('[hello]'),
            expect.poll(() => missingPage.textContent('.hmr')).toBe('[hello]'),
          ]);

          const [loadedPatch, missingPatch] = await Promise.all([
            readHmrPatch(loadedPagePatches.urls),
            readHmrPatch(missingPagePatches.urls),
          ]);
          expect(loadedPatch).not.toContain('//#region facade.js');
          expect(missingPatch).toContain('//#region facade.js');
          expect(missingPatch).toContain('//#region implementation.js');
        } finally {
          loadedPagePatches.stop();
          missingPagePatches.stop();
          await waitForBuildStable(CONFIG.ports.hmrFullBundleMode);
          await editFile('hmr.js', (code) => code.replace(FACADE_RENDER, IDENTITY_RENDER));
        }

        await Promise.all([
          expect.poll(() => loadedPage.textContent('.hmr')).toBe('hello'),
          expect.poll(() => missingPage.textContent('.hmr')).toBe('hello'),
        ]);
      } finally {
        await missingContext.close();
      }

      await waitForBuildStable(CONFIG.ports.hmrFullBundleMode);
    },
  );

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

// Regression for vitejs/vite#22454. With `experimental.devMode.lazy: true` and
// `viteAliasPlugin`, `import('@lazy')` used to produce a proxy module whose id
// carried `?rolldown-lazy=1?rolldown-lazy=1` (suffix appended twice). The
// doubled key broke `delete __rolldown_runtime__.modules[$STABLE_PROXY_MODULE_ID]`
// in the proxy template (the template emits a SINGLE-suffix key), the dedup
// gate skipped the fetched-template re-execution, the real module never
// registered its named exports, and `mod.foo` / `mod.bar` came back undefined.
// The fix in `crates/rolldown_plugin_lazy_compilation/src/lazy_compilation_plugin.rs::resolve_id`
// makes the marker append idempotent.
describe('lazy-aliased-import', () => {
  test.sequential(
    'aliased dynamic import resolves named exports (#vite-22454)',
    { retry: 0 },
    async () => {
      const page = getLazyAliasedImportPage();

      await page.goto(LAZY_ALIASED_IMPORT_URL, { waitUntil: 'domcontentloaded' });
      await page.click('#btn');
      await expect.poll(() => page.textContent('#status')).toBe('done');

      const log = (await page.textContent('#log')) ?? '';
      expect(log).toContain('mod.foo = lazy_foo');
      expect(log).toContain('mod.bar = lazy_bar');
      expect(log).not.toContain('UNDEFINED');
    },
  );
});
