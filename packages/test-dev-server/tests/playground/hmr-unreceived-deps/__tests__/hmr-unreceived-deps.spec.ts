import type { Page, Response } from 'playwright';
import { afterAll, describe, expect, test } from 'vitest';
import { browser, editFile, page, serverUrl, waitForBuildStable } from '~utils';

// Four tabs of the same app. Tab A (the shared `page`) and tab R click `.load`,
// so lazy-holder.js and its imports enter the server's module graph and only
// those two tabs run them. Tab R never acknowledges the payloads it runs
// (`payloadDelivered` is stubbed), so the server never records those modules as
// delivered to it. Tabs B and C never click.
//
// All tabs open before the first edit: opening a page after an HMR update
// regenerates the bundle and reloads every tab.

const IDENTITY_RENDER = 'export const render = (value) => value;';
const FACADE_RENDER = "import { decorate } from './facade.js';\nexport const render = decorate;";
const CJS_FACADE_RENDER =
  "import cjsFacade from './cjs-facade.cjs';\nexport const render = cjsFacade.decorate;";
const CONDITIONAL_CJS_FACADE_RENDER =
  "import conditionalCjsFacade from './conditional-cjs-facade.cjs';\n" +
  'export const render = conditionalCjsFacade.decorate;';
const LAZY_TEXT = '[a] [cjs:b] [conditional:c] dynamic-existing';

type Edit = [file: string, from: string, to: string];

let tabB: Page;
let tabC: Page;
let tabR: Page;

const readGlobal = (p: Page, name: string) => p.evaluate((n) => (globalThis as any)[n], name);
const setGlobal = (p: Page, name: string) =>
  p.evaluate((n) => ((globalThis as any)[n] = true), name);

/** Plant a marker on `window`; any full page reload wipes it. */
const plantMarker = (p: Page) => p.evaluate(() => ((window as any).__marker = 'alive'));

const errors = new Map<Page, string[]>();

function trackErrors(p: Page) {
  const list: string[] = [];
  errors.set(p, list);
  p.on('console', (msg) => {
    if (msg.type() === 'error') list.push(msg.text());
  });
  p.on('pageerror', (error) => list.push(error.message));
}

async function expectNoReloadOrError() {
  for (const p of [page, tabB, tabC, tabR]) {
    expect(errors.get(p)).toEqual([]);
    expect(await p.evaluate(() => (window as any).__marker ?? null)).toBe('alive');
  }
}

const factoryFor = (file: string) => new RegExp(`registerFactory\\("[^"]*/${file}"`);

/** Collect the bodies of the HMR patches a tab fetches. */
function collectPatches(p: Page) {
  const bodies: Promise<string>[] = [];
  const onResponse = (res: Response) => {
    if (/\/hmr_patch_\d+\.js(?:\?|$)/.test(res.url())) bodies.push(res.text());
  };
  p.on('response', onResponse);
  return {
    read: async () => (await Promise.all(bodies)).join('\n'),
    stop: () => p.off('response', onResponse),
  };
}

async function openTab(): Promise<Page> {
  const tab = await browser.newPage();
  trackErrors(tab);
  await tab.goto(serverUrl);
  await expect.poll(() => tab.textContent('.hmr')).toBe('hello');
  return tab;
}

/** Apply `edits`, run `check`, then revert them so the next test starts from the original files. */
async function withEdits(edits: Edit[], check: () => Promise<void>) {
  for (const [file, from, to] of edits) {
    editFile(file, (code) => code.replace(from, to));
  }
  try {
    await check();
  } finally {
    await waitForBuildStable();
    for (const [file, from, to] of edits) {
      editFile(file, (code) => code.replace(to, from));
    }
    await waitForBuildStable();
  }
}

afterAll(async () => {
  await Promise.all([tabB, tabC, tabR].map((tab) => tab?.close()));
});

describe('hmr-unreceived-deps', () => {
  test('tabs A and R run the lazy modules', async () => {
    trackErrors(page);
    await waitForBuildStable();
    [tabB, tabC, tabR] = await Promise.all([openTab(), openTab(), openTab()]);
    await waitForBuildStable();

    await tabR.evaluate(() => {
      (globalThis as any).__rolldown_runtime__.payloadDelivered = () => {};
    });
    await page.click('.load');
    await expect.poll(() => page.textContent('.lazy')).toBe(LAZY_TEXT);
    await tabR.click('.load');
    await expect.poll(() => tabR.textContent('.lazy')).toBe(LAZY_TEXT);
    await tabR.click('.load-late');
    await expect.poll(() => tabR.textContent('.late')).toBe('late');
    await waitForBuildStable();

    await Promise.all([page, tabB, tabC, tabR].map(plantMarker));
  });

  test('a new import of an ESM re-export chain', async () => {
    const patchesA = collectPatches(page);
    const patchesR = collectPatches(tabR);
    try {
      await withEdits([['hmr.js', IDENTITY_RENDER, FACADE_RENDER]], async () => {
        for (const p of [page, tabB, tabC, tabR]) {
          await expect.poll(() => p.textContent('.hmr')).toBe('[hello]');
        }
        // The server sends implementation.js again only to tab R, which never acknowledged it.
        expect(await patchesR.read()).toMatch(factoryFor('implementation.js'));
        expect(await patchesA.read()).not.toMatch(factoryFor('implementation.js'));
        // Tab R already ran implementation.js, so the copy sent again must not run.
        expect(await readGlobal(tabR, '__implementationExecutions')).toBe(1);
        await expectNoReloadOrError();
      });
    } finally {
      patchesA.stop();
      patchesR.stop();
    }
  });

  test('a new import of a CommonJS module that requires another', async () => {
    await withEdits([['hmr.js', IDENTITY_RENDER, CJS_FACADE_RENDER]], async () => {
      for (const p of [page, tabB, tabC, tabR]) {
        await expect.poll(() => p.textContent('.hmr')).toBe('[cjs:hello]');
      }
      await expectNoReloadOrError();
    });
  });

  test('a conditional require runs only when its condition holds', async () => {
    await setGlobal(tabC, '__loadConditionalCjs');
    await withEdits([['hmr.js', IDENTITY_RENDER, CONDITIONAL_CJS_FACADE_RENDER]], async () => {
      for (const p of [page, tabB, tabC, tabR]) {
        await expect.poll(() => p.textContent('.hmr')).toBe('[conditional:hello]');
      }
      expect(await readGlobal(tabB, '__conditionalCjsExecutions')).toBeUndefined();
      expect(await readGlobal(tabC, '__conditionalCjsExecutions')).toBe(1);
      await expectNoReloadOrError();
    });
  });

  test('a re-run module dynamically imports a module the tab never received', async () => {
    await setGlobal(tabB, '__loadDynamicExisting');
    await withEdits([['dynamic-importer.js', "'initial'", "'updated'"]], async () => {
      await expect.poll(() => readGlobal(tabB, '__dynamicExistingValue')).toBe('dynamic-existing');
      await expectNoReloadOrError();
    });
  });

  test('a self-accepting module whose delivery the tab never acknowledged', async () => {
    await withEdits(
      [['late-accept.js', "export const value = 'late';", "export const value = 'late2';"]],
      async () => {
        await expect.poll(() => readGlobal(tabR, '__lateAcceptValue')).toBe('late2');
        expect(await readGlobal(page, '__lateAcceptValue')).toBeUndefined();
        await expectNoReloadOrError();
      },
    );
  });

  test('a newly imported module whose dependency changed in the same update', async () => {
    const executions = await readGlobal(tabR, '__implementationExecutions');
    await withEdits(
      [
        ['implementation.js', 'return `[${value}]`;', 'return `<${value}>`;'],
        ['hmr.js', IDENTITY_RENDER, FACADE_RENDER],
      ],
      async () => {
        for (const p of [page, tabB, tabC, tabR]) {
          await expect.poll(() => p.textContent('.hmr')).toBe('<hello>');
        }
        // Tab R ran the old copy of implementation.js: it runs the new copy once.
        expect(await readGlobal(tabR, '__implementationExecutions')).toBe(executions + 1);
        await expectNoReloadOrError();
      },
    );
  });
});
