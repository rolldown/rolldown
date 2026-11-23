import { getDevWatchOptionsForCi } from '@rolldown/test-dev-server';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import type { InputOptions, OutputOptions, RolldownOutput } from 'rolldown';
import type { DevEngine, DevOptions } from 'rolldown/experimental';
import { dev as _dev } from 'rolldown/experimental';
import { isSingleThread } from '@tests/runtime-flavor';
import { sleep } from 'rolldown-tests/utils';
import { test, vi } from 'vitest';

const TEST_RETRY = 3;
const TEST_TIMEOUT = 60_000;

// Every test skips on CurrentThread: `dev()` rejects there (`devSupported`).

// Wrap dev() to inject usePolling for CI stability.
// PollWatcher uses whole-second mtime comparison, so file edits
// must use editFile() to ensure mtime crosses a second boundary.
function dev(
  inputOptions: InputOptions,
  outputOptions: OutputOptions,
  devOptions: DevOptions,
): Promise<DevEngine> {
  return _dev(inputOptions, outputOptions, {
    ...devOptions,
    watch: {
      ...devOptions.watch,
      watcher: {
        ...getDevWatchOptionsForCi().watcher,
        ...devOptions.watch?.watcher,
      },
    },
  });
}

// Write a file with a 1s sleep beforehand to ensure the PollWatcher's
// whole-second mtime comparison detects the change.
async function editFile(filePath: string, content: string) {
  await sleep(1000);
  fs.writeFileSync(filePath, content);
}

test
  .skipIf(isSingleThread)
  .concurrent(
    'dev watch exclude',
    { retry: TEST_RETRY, timeout: TEST_TIMEOUT },
    async ({ task, expect, onTestFinished }) => {
      const retryCount = task.result?.retryCount ?? 0;
      const { input, outputDir, dir } = createTestInputAndOutput('dev-include-exclude', retryCount);

      const onOutput = vi.fn();
      const onHmrUpdates = vi.fn();
      const engine = await dev(
        {
          input,
          experimental: { devMode: true },
        },
        { dir: outputDir },
        {
          onOutput,
          onHmrUpdates,
          watch: {
            exclude: '**/main.js',
          },
        },
      );
      onTestFinished(async () => {
        await engine.close();
        if (!process.env.CI) {
          fs.rmSync(dir, { recursive: true, force: true });
        }
      });

      await engine.run();
      await expect.poll(() => onOutput).toHaveBeenCalled();

      // edit file
      onOutput.mockClear();
      onHmrUpdates.mockClear();
      await editFile(input, 'console.log(2)');
      // The input is excluded, so no rebuild or HMR update should fire.
      await sleep(1000);
      expect(onOutput).not.toHaveBeenCalled();
      expect(onHmrUpdates).not.toHaveBeenCalled();
    },
  );

test
  .skipIf(isSingleThread)
  .concurrent(
    'dev watch exclude sanity',
    { retry: TEST_RETRY, timeout: TEST_TIMEOUT },
    async ({ task, expect, onTestFinished }) => {
      const retryCount = task.result?.retryCount ?? 0;
      const { input, outputDir, dir } = createTestInputAndOutput(
        'dev-include-exclude-sanity',
        retryCount,
      );

      const onOutput = vi.fn();
      const onHmrUpdates = vi.fn();
      const engine = await dev(
        {
          input,
          experimental: { devMode: true },
        },
        { dir: outputDir },
        {
          onOutput,
          onHmrUpdates,
          watch: {
            exclude: '**/unrelated.js',
          },
        },
      );
      onTestFinished(async () => {
        await engine.close();
        if (!process.env.CI) {
          fs.rmSync(dir, { recursive: true, force: true });
        }
      });

      await engine.run();
      await expect.poll(() => onOutput).toHaveBeenCalled();

      // edit file
      onHmrUpdates.mockClear();
      await editFile(input, 'console.log(2)');
      // The input is not excluded, so an HMR update should fire.
      await expect.poll(() => onHmrUpdates).toHaveBeenCalled();
    },
  );

test
  .skipIf(isSingleThread)
  .concurrent(
    'dev watch include',
    { retry: TEST_RETRY, timeout: TEST_TIMEOUT },
    async ({ task, expect, onTestFinished }) => {
      const retryCount = task.result?.retryCount ?? 0;
      const { dir: cwd } = createTestWithMultiFiles('dev-include', retryCount, {
        'main.js': `import './dep.js'\nconsole.log(1)`,
        'dep.js': `export const dep = 1`,
      });

      const onOutput = vi.fn();
      const onHmrUpdates = vi.fn();
      const engine = await dev(
        {
          cwd,
          input: 'main.js',
          experimental: { devMode: true },
        },
        { dir: path.join(cwd, 'dist') },
        {
          onOutput,
          onHmrUpdates,
          watch: {
            // Only main.js is watched; dep.js falls outside the allowlist.
            include: '**/main.js',
          },
        },
      );
      onTestFinished(async () => {
        await engine.close();
        if (!process.env.CI) {
          fs.rmSync(cwd, { recursive: true, force: true });
        }
      });

      await engine.run();
      await expect.poll(() => onOutput).toHaveBeenCalled();

      // edit file not matching include
      onHmrUpdates.mockClear();
      await editFile(path.join(cwd, 'dep.js'), `export const dep = 2`);
      // dep.js is outside the include allowlist, so no HMR update should fire.
      await sleep(1000);
      expect(onHmrUpdates).not.toHaveBeenCalled();

      // edit file matching include
      await editFile(path.join(cwd, 'main.js'), `import './dep.js'\nconsole.log(2)`);
      // main.js matches include, so an HMR update should fire.
      await expect.poll(() => onHmrUpdates).toHaveBeenCalled();
    },
  );

test
  .skipIf(isSingleThread)
  .concurrent(
    'onOutput should be called only once when initial build fails',
    { retry: TEST_RETRY, timeout: TEST_TIMEOUT },
    async ({ task, expect, onTestFinished }) => {
      const retryCount = task.result?.retryCount ?? 0;
      const { input, outputDir, dir } = createTestInputAndOutput(
        'dev-initial-build-error',
        retryCount,
      );

      // Write a syntax error to the input file
      fs.writeFileSync(input, 'const x = {;');

      const onOutput = vi.fn();
      const engine = await dev(
        {
          input,
          experimental: { devMode: true },
        },
        { dir: outputDir },
        { onOutput },
      );
      onTestFinished(async () => {
        await engine.close();
        if (!process.env.CI) {
          fs.rmSync(dir, { recursive: true, force: true });
        }
      });

      await engine.run();
      expect(onOutput).toHaveBeenCalledTimes(1);
      // The single call should be an error
      expect(onOutput.mock.calls[0][0]).toBeInstanceOf(Error);
    },
  );

// An HMR patch's factories are re-evaluation demand: only modules the client may
// re-run belong in it. When an edit makes a hot module import a module the entry
// chunk already evaluated at top level, the patch must carry the edited module's
// factory only — the new import is served by the live exports already registered
// on the client (`initModule` is registry-gated), never by re-shipping a factory.
test
  .skipIf(isSingleThread)
  .concurrent(
    'HMR patch does not ship factories for a newly imported top-level-evaluated module',
    { retry: TEST_RETRY, timeout: TEST_TIMEOUT },
    async ({ task, expect, onTestFinished }) => {
      const retryCount = task.result?.retryCount ?? 0;
      const { dir: cwd } = createTestWithMultiFiles('dev-patch-top-level-evaluated', retryCount, {
        'main.js': `import './lib.js';\nimport './hot.js';\n`,
        'lib.js': `export const value = 'lib';\n`,
        'hot.js': `export const tag = 'hot';\nimport.meta.hot.accept();\n`,
      });

      const onHmrUpdates = vi.fn();
      const engine = await dev(
        {
          cwd,
          input: './main.js',
          experimental: { devMode: true },
        },
        { dir: path.join(cwd, 'dist') },
        { onHmrUpdates },
      );
      onTestFinished(async () => {
        await engine.close();
        if (!process.env.CI) {
          fs.rmSync(cwd, { recursive: true, force: true });
        }
      });

      await engine.run();
      // The hello: updates are computed per registered client.
      await engine.registerClient('registered-client');

      // The edit adds an import of `lib.js`, which the entry chunk already
      // evaluated at top level (`main.js` imports it statically).
      await editFile(
        path.join(cwd, 'hot.js'),
        `import { value } from './lib.js';\nexport const tag = 'hot-' + value;\nimport.meta.hot.accept();\n`,
      );

      const findPatch = () =>
        onHmrUpdates.mock.calls
          .flatMap(([result]) => (result instanceof Error ? [] : result.updates))
          .find((u) => u.clientId === 'registered-client' && u.update.type === 'Patch');
      await expect.poll(findPatch, { timeout: 20_000 }).toBeTruthy();

      const patch = findPatch()!.update as { type: 'Patch'; code: string };
      // The edited module re-runs, so its factory ships.
      expect(patch.code).toMatch(/registerFactory\("[^"]*hot\.js"/);
      // The newly imported module stays live on the client; no factory for it.
      expect(patch.code).not.toMatch(/registerFactory\("[^"]*lib\.js"/);
      // The re-run resolves the new import through the registry instead.
      expect(patch.code).toMatch(/initModule\("[^"]*lib\.js"\)/);
    },
  );

test
  .skipIf(isSingleThread)
  .concurrent(
    'HMR patch includes a factory for an initially unloaded barrel',
    { retry: TEST_RETRY, timeout: TEST_TIMEOUT },
    async ({ task, expect, onTestFinished }) => {
      const retryCount = task.result?.retryCount ?? 0;
      const { dir: cwd } = createTestWithMultiFiles('dev-patch-barrel-factory', retryCount, {
        'main.js': `import { render } from './view.js';\nconsole.log(render());\n`,
        'view.js': `import { format } from './barrel.js';\nexport const render = () => format('before');\nimport.meta.hot.accept();\n`,
        'barrel.js': `export * from './implementation.js';\n`,
        'implementation.js': `export const format = value => 'rendered:' + value;\n`,
      });

      const onHmrUpdates = vi.fn();
      const engine = await dev(
        {
          cwd,
          input: './main.js',
          experimental: { devMode: true },
          optimization: { inlineConst: false },
          plugins: [
            {
              name: 'side-effect-free-barrel',
              resolveId(source) {
                if (source === './barrel.js') {
                  return { id: path.join(cwd, 'barrel.js'), moduleSideEffects: false };
                }
              },
            },
          ],
        },
        {
          dir: path.join(cwd, 'dist'),
          codeSplitting: {
            groups: [
              {
                name: 'barrel',
                test: /barrel\.js$/,
                includeDependenciesRecursively: false,
                entriesAware: false,
                minShareCount: 1,
              },
            ],
          },
        },
        { onHmrUpdates },
      );
      onTestFinished(async () => {
        await engine.close();
        if (!process.env.CI) {
          fs.rmSync(cwd, { recursive: true, force: true });
        }
      });

      await engine.run();
      await engine.registerClient('barrel-client');
      const view = path.join(cwd, 'view.js');
      await editFile(view, fs.readFileSync(view, 'utf8').replace("'before'", "'after'"));

      const findPatch = () =>
        onHmrUpdates.mock.calls
          .flatMap(([result]) => (result instanceof Error ? [] : result.updates))
          .find((u) => u.clientId === 'barrel-client' && u.update.type === 'Patch');
      await expect.poll(findPatch, { timeout: 20_000 }).toBeTruthy();

      const patch = findPatch()!.update as { type: 'Patch'; code: string };
      expect(patch.code).toMatch(/registerFactory\("[^"]*barrel\.js"/);
    },
  );

// A lazy proxy never runs in the client: `requestLazy` runs the real module. So no patch
// carries a factory for a proxy. Two edits could bring one in: the real module changes its
// interop, which the proxy's own `import()` of it reads, and a new `import()` creates a new
// proxy.
test
  .skipIf(isSingleThread)
  .concurrent(
    'an HMR patch carries no factory for a lazy proxy',
    { retry: TEST_RETRY, timeout: TEST_TIMEOUT },
    async ({ task, expect, onTestFinished }) => {
      const retryCount = task.result?.retryCount ?? 0;
      const { dir: cwd } = createTestWithMultiFiles('dev-lazy-proxy-not-in-patch', retryCount, {
        'main.js': `import('./lazy.js');\n`,
        'lazy.js': `export const value = 'esm';\n`,
        'other.js': `export const other = 'other';\n`,
      });

      const onHmrUpdates = vi.fn();
      const engine = await dev(
        {
          cwd,
          input: './main.js',
          experimental: { devMode: { lazy: true } },
        },
        { dir: path.join(cwd, 'dist') },
        { onHmrUpdates },
      );
      onTestFinished(async () => {
        await engine.close();
        if (!process.env.CI) {
          fs.rmSync(cwd, { recursive: true, force: true });
        }
      });

      await engine.run();
      await engine.registerClient('client');
      const chunk = await engine.compileEntry(
        `${path.join(cwd, 'lazy.js')}?rolldown-lazy=1`,
        'client',
      );
      await engine.notifyPayloadDelivered(chunk.filename);
      await engine.ensureLatestBuildOutput();

      const patchWith = (code: unknown) =>
        expect.objectContaining({
          updates: expect.arrayContaining([
            expect.objectContaining({
              clientId: 'client',
              update: expect.objectContaining({ type: 'Patch', code }),
            }),
          ]),
        });
      const lazyFactory = expect.stringMatching(/registerFactory\("[^"]*lazy\.js"/);
      const proxyFactory = expect.stringMatching(/registerFactory\("[^"]*\?rolldown-lazy=1"/);

      // ESM to CommonJS
      onHmrUpdates.mockClear();
      await editFile(path.join(cwd, 'lazy.js'), `module.exports = { value: 'cjs' };\n`);
      await expect
        .poll(() => onHmrUpdates, { timeout: 20_000 })
        .toHaveBeenCalledWith(patchWith(lazyFactory));
      expect(onHmrUpdates).not.toHaveBeenCalledWith(patchWith(proxyFactory));

      // a new lazy `import()`, so a new proxy
      onHmrUpdates.mockClear();
      await editFile(
        path.join(cwd, 'lazy.js'),
        `module.exports = { value: 'cjs', load: () => import('./other.js') };\n`,
      );
      await expect
        .poll(() => onHmrUpdates, { timeout: 20_000 })
        .toHaveBeenCalledWith(patchWith(lazyFactory));
      expect(onHmrUpdates).not.toHaveBeenCalledWith(patchWith(proxyFactory));
    },
  );

// https://github.com/rolldown/rolldown/issues/10487
//
// Deleting a still-imported file must fail the round with an unresolved-import
// error, and recreating the file must recover. This matches Vite's bundled dev
// and a cold build (covered end-to-end by the `hmr-delete-self-watched`
// dev-server playground). oxc_resolver caches every filesystem lookup, so the
// engine clears the resolver cache when a batch creates or deletes a file.
test
  .skipIf(isSingleThread)
  .concurrent(
    'deleting an imported file surfaces a resolve error and recreating it recovers',
    { retry: TEST_RETRY, timeout: TEST_TIMEOUT },
    async ({ task, expect, onTestFinished }) => {
      const retryCount = task.result?.retryCount ?? 0;
      const { dir: cwd } = createTestWithMultiFiles('dev-delete-imported-file', retryCount, {
        'main.js': `import './parent.js';\n`,
        'parent.js': `import { value } from './child.js';\nexport const childValue = value;\nexport const parentValue = 'parent';\nimport.meta.hot.accept();\n`,
        'child.js': `export const value = 'child';\n`,
      });

      const onHmrUpdates = vi.fn();
      const engine = await dev(
        {
          cwd,
          input: './main.js',
          experimental: { devMode: true },
        },
        { dir: path.join(cwd, 'dist') },
        { onHmrUpdates },
      );
      onTestFinished(async () => {
        await engine.close();
        if (!process.env.CI) {
          fs.rmSync(cwd, { recursive: true, force: true });
        }
      });

      await engine.run();
      await engine.registerClient('client');

      const errorCalls = () =>
        onHmrUpdates.mock.calls
          .map(([result]) => result)
          .filter((result): result is Error => result instanceof Error);
      const patches = () =>
        onHmrUpdates.mock.calls
          .flatMap(([result]) => (result instanceof Error ? [] : result.updates))
          .filter((u) => u.clientId === 'client' && u.update.type === 'Patch');

      // Delete the imported file: the round must fail with an unresolved import,
      // exactly like a cold build of the same file state.
      await sleep(1000);
      onHmrUpdates.mockClear();
      fs.rmSync(path.join(cwd, 'child.js'));
      await expect.poll(() => errorCalls().length, { timeout: 20_000 }).toBeGreaterThan(0);
      expect(String(errorCalls()[0])).toMatch(/child\.js/);

      // Recreate the file with changed content: recovery must ship the new
      // content to the client.
      onHmrUpdates.mockClear();
      await editFile(path.join(cwd, 'child.js'), `export const value = 'child2';\n`);
      await expect.poll(() => patches().length, { timeout: 20_000 }).toBeGreaterThan(0);
      const patch = patches()[0].update as { type: 'Patch'; code: string };
      expect(patch.code).toContain('child2');
    },
  );

function createTestInputAndOutput(testLabel: string, retryCount: number) {
  const uniqueId = crypto.randomUUID().slice(0, 8);
  const dirname = `${testLabel}-${uniqueId}-retry${retryCount}`;
  const dir = path.join(import.meta.dirname, 'temp', dirname);
  fs.mkdirSync(dir, { recursive: true });
  const input = path.join(dir, 'main.js');
  fs.writeFileSync(input, 'console.log(1)');
  const outputDir = path.join(dir, 'dist');
  return { input, outputDir, dir };
}

test
  .skipIf(isSingleThread)
  .concurrent(
    'dev re-emitted asset with changed content is included in rebuild output',
    { retry: TEST_RETRY, timeout: TEST_TIMEOUT },
    async ({ task, expect, onTestFinished }) => {
      const retryCount = task.result?.retryCount ?? 0;
      const { input, outputDir, dir } = createTestInputAndOutput('dev-reemit-asset', retryCount);

      const outputs: RolldownOutput[] = [];
      const findAsset = (output: RolldownOutput) =>
        output.output.find(
          (o): o is Extract<typeof o, { type: 'asset' }> =>
            o.type === 'asset' && o.fileName === 'extra.html',
        );

      const engine = await dev(
        {
          input,
          experimental: { devMode: true },
          plugins: [
            {
              name: 'emit-asset',
              generateBundle() {
                // Re-emit the same fileName on every build; its content changes
                // whenever the input module is edited.
                this.emitFile({
                  type: 'asset',
                  fileName: 'extra.html',
                  source: `<script>${fs.readFileSync(input, 'utf8')}</script>`,
                });
              },
            },
          ],
        },
        { dir: outputDir },
        {
          onOutput: (result) => {
            if (!(result instanceof Error)) {
              outputs.push(result);
            }
          },
        },
      );
      onTestFinished(async () => {
        await engine.close();
        if (!process.env.CI) {
          fs.rmSync(dir, { recursive: true, force: true });
        }
      });

      await engine.run();
      await expect.poll(() => outputs.length).toBeGreaterThan(0);
      expect(findAsset(outputs.at(-1)!)?.source).toContain('console.log(1)');

      outputs.length = 0;
      await editFile(input, 'console.log(2)');
      await expect
        .poll(async () => {
          await engine.ensureLatestBuildOutput();
          const output = outputs.at(-1);
          return output ? findAsset(output)?.source : undefined;
        })
        .toContain('console.log(2)');
    },
  );

function createTestWithMultiFiles(
  testLabel: string,
  retryCount: number,
  files: Record<string, string>,
) {
  const uniqueId = crypto.randomUUID().slice(0, 8);
  const dirname = `${testLabel}-${uniqueId}-retry${retryCount}`;
  const dir = path.join(import.meta.dirname, 'temp', dirname);
  fs.mkdirSync(dir, { recursive: true });
  for (const [fileName, content] of Object.entries(files)) {
    fs.writeFileSync(path.join(dir, fileName), content);
  }
  return { dir };
}

// https://github.com/rolldown/rolldown/issues/11223
test
  .skipIf(isSingleThread)
  .concurrent(
    'a partial scan resets the module options of a changed module to those of its resolution',
    { retry: TEST_RETRY, timeout: TEST_TIMEOUT },
    async ({ task, expect, onTestFinished }) => {
      const retryCount = task.result?.retryCount ?? 0;
      const { input, outputDir, dir } = createTestInputAndOutput('dev-module-options', retryCount);
      const source = path.join(dir, 'mod-source.js');
      fs.writeFileSync(input, `import 'virtual:mod'`);
      fs.writeFileSync(source, 'export const value = 1');

      const infos: unknown[] = [];
      const outputs: RolldownOutput[] = [];
      const outputCode = () =>
        outputs
          .at(-1)
          ?.output.map((chunk) => (chunk.type === 'chunk' ? chunk.code : ''))
          .join('\n');

      const engine = await dev(
        {
          input,
          plugins: [
            {
              name: 'toggle-side-effects',
              resolveId: (id) =>
                id === 'virtual:mod'
                  ? { id: '\0virtual:mod', meta: { resolved: true } }
                  : undefined,
              load(id) {
                if (id !== '\0virtual:mod') return;
                this.addWatchFile(source);
                return fs.readFileSync(source, 'utf8');
              },
              transform(code, id) {
                if (id !== '\0virtual:mod') return;
                return code.includes('EFFECT') ? { code } : { code, moduleSideEffects: false };
              },
              moduleParsed({ id }) {
                if (id !== '\0virtual:mod') return;
                const { meta, moduleSideEffects } = this.getModuleInfo(id)!;
                infos.push({ meta: { ...meta }, moduleSideEffects });
              },
            },
          ],
        },
        { dir: outputDir },
        {
          // Without `devMode`, an edit only rebuilds when the strategy is `always`.
          rebuildStrategy: 'always',
          onOutput: (result) => {
            if (!(result instanceof Error)) outputs.push(result);
          },
        },
      );
      onTestFinished(async () => {
        await engine.close();
        if (!process.env.CI) {
          fs.rmSync(dir, { recursive: true, force: true });
        }
      });

      await engine.run();
      await expect.poll(() => outputs.length).toBeGreaterThan(0);
      expect(outputCode()).not.toContain('EFFECT');
      expect(infos).toEqual([{ meta: { resolved: true }, moduleSideEffects: false }]);

      await editFile(source, 'globalThis.EFFECT = true\nexport const value = 1');
      await expect
        .poll(async () => {
          await engine.ensureLatestBuildOutput();
          return outputCode();
        })
        .toContain('EFFECT');
      // Only the options from `resolveId` stay.
      expect(infos.at(-1)).toEqual({ meta: { resolved: true }, moduleSideEffects: true });
    },
  );

test
  .skipIf(isSingleThread)
  .concurrent(
    'an HMR refetch keeps the moduleSideEffects of the resolution that created the module',
    { retry: TEST_RETRY, timeout: TEST_TIMEOUT },
    async ({ task, expect, onTestFinished }) => {
      const retryCount = task.result?.retryCount ?? 0;
      const { input, outputDir, dir } = createTestInputAndOutput(
        'dev-refetch-side-effects',
        retryCount,
      );
      const dep = path.join(dir, 'dep.js');
      fs.writeFileSync(input, `import './dep.js'\nconsole.log('MAIN')`);
      fs.writeFileSync(dep, `console.log('EFFECT_1')`);

      const outputs: string[] = [];
      const onHmrUpdates = vi.fn();
      let depLoads = 0;
      const engine = await dev(
        {
          input,
          plugins: [
            {
              name: 'side-effect-free-dep',
              resolveId: (source) =>
                source === './dep.js' ? { id: dep, moduleSideEffects: false } : undefined,
              load(id) {
                if (id === dep) depLoads++;
              },
            },
          ],
        },
        { dir: outputDir },
        {
          // The edit runs only the HMR stage, so `ensureLatestBuildOutput` shows the module that
          // the HMR stage fetched again.
          rebuildStrategy: 'never',
          onHmrUpdates,
          onOutput: (result) => {
            if (!(result instanceof Error)) {
              outputs.push(
                result.output.map((chunk) => (chunk.type === 'chunk' ? chunk.code : '')).join('\n'),
              );
            }
          },
        },
      );
      onTestFinished(async () => {
        await engine.close();
        if (!process.env.CI) {
          fs.rmSync(dir, { recursive: true, force: true });
        }
      });

      await engine.run();
      await expect.poll(() => outputs.length).toBe(1);
      expect(outputs[0]).toContain('MAIN');
      expect(outputs[0]).not.toContain('EFFECT_1');

      await editFile(dep, `console.log('EFFECT_2')`);
      await expect.poll(() => onHmrUpdates).toHaveBeenCalled();
      expect(depLoads).toBe(2);
      await engine.ensureLatestBuildOutput();
      await expect.poll(() => outputs.length).toBe(2);
      expect(outputs[1]).toContain('MAIN');
      expect(outputs[1]).not.toContain('EFFECT_2');
    },
  );

function chunkCode(output: RolldownOutput): string {
  return output.output.map((chunk) => (chunk.type === 'chunk' ? chunk.code : '')).join('\n');
}

test
  .skipIf(isSingleThread)
  .concurrent(
    'an incremental rebuild keeps the moduleSideEffects of the resolution that created the module',
    { retry: TEST_RETRY, timeout: TEST_TIMEOUT },
    async ({ task, expect, onTestFinished }) => {
      const retryCount = task.result?.retryCount ?? 0;
      const { input, outputDir, dir } = createTestInputAndOutput(
        'dev-rebuild-side-effects',
        retryCount,
      );
      const dep = path.join(dir, 'dep.js');
      fs.writeFileSync(input, `import './dep.js'\nconsole.log('MAIN')`);
      fs.writeFileSync(dep, `console.log('EFFECT_1')`);

      const outputs: string[] = [];
      let depLoads = 0;
      const engine = await dev(
        {
          input,
          plugins: [
            {
              name: 'side-effect-free-dep',
              // The rebuild resolves `dep.js` again by its path, without an importer. This hook
              // does not handle that resolution.
              resolveId: (source) =>
                source === './dep.js' ? { id: dep, moduleSideEffects: false } : undefined,
              load(id) {
                if (id === dep) depLoads++;
              },
            },
          ],
        },
        { dir: outputDir },
        {
          rebuildStrategy: 'always',
          onOutput: (result) => {
            if (!(result instanceof Error)) outputs.push(chunkCode(result));
          },
        },
      );
      onTestFinished(async () => {
        await engine.close();
        if (!process.env.CI) {
          fs.rmSync(dir, { recursive: true, force: true });
        }
      });

      await engine.run();
      await expect.poll(() => outputs.length).toBe(1);
      expect(outputs[0]).toContain('MAIN');
      expect(outputs[0]).not.toContain('EFFECT_1');

      await editFile(dep, `console.log('EFFECT_2')`);
      await expect.poll(() => outputs.length).toBe(2);
      // The first build, the HMR stage and the rebuild.
      expect(depLoads).toBe(3);
      expect(outputs[1]).toContain('MAIN');
      expect(outputs[1]).not.toContain('EFFECT_2');
    },
  );

test
  .skipIf(isSingleThread)
  .concurrent(
    'an incremental rebuild resolves a changed module again with its start options',
    { retry: TEST_RETRY, timeout: TEST_TIMEOUT },
    async ({ task, expect, onTestFinished }) => {
      const retryCount = task.result?.retryCount ?? 0;
      const { input, outputDir, dir } = createTestInputAndOutput(
        'dev-rebuild-resolve-options',
        retryCount,
      );
      const dep = path.join(dir, 'dep.js');
      fs.writeFileSync(input, `import './dep.js'\nconsole.log('MAIN')`);
      fs.writeFileSync(dep, `console.log('EFFECT_1')`);

      const outputs: string[] = [];
      const seenByResolve: unknown[] = [];
      let inHmrStage = false;
      const engine = await dev(
        {
          input,
          plugins: [
            {
              name: 'test-plugin',
              buildStart() {
                inHmrStage = false;
              },
              resolveId(source, importer) {
                if (source === './dep.js' && importer) {
                  return { id: dep, meta: { creator: true } };
                }
                if (source === dep && !importer) {
                  const { meta, moduleSideEffects } = this.getModuleInfo(dep)!;
                  seenByResolve.push({ meta: { ...meta }, moduleSideEffects });
                  return { id: dep };
                }
              },
              transform(code, id) {
                if (id === dep && inHmrStage) {
                  return { code, meta: { fromHmrStage: true }, moduleSideEffects: false };
                }
              },
            },
          ],
        },
        { dir: outputDir },
        {
          rebuildStrategy: 'always',
          onOutput: (result) => {
            if (!(result instanceof Error)) outputs.push(chunkCode(result));
          },
        },
      );
      onTestFinished(async () => {
        await engine.close();
        if (!process.env.CI) {
          fs.rmSync(dir, { recursive: true, force: true });
        }
      });

      await engine.run();
      await expect.poll(() => outputs.length).toBe(1);
      expect(outputs[0]).toContain('EFFECT_1');

      inHmrStage = true;
      await editFile(dep, `console.log('EFFECT_2')`);
      await expect.poll(() => outputs.length).toBe(2);
      expect.soft(seenByResolve).toEqual([{ meta: { creator: true }, moduleSideEffects: true }]);
      expect.soft(outputs[1]).toContain('EFFECT_2');
    },
  );

test
  .skipIf(isSingleThread)
  .concurrent(
    'a rescan after a failed build keeps the moduleSideEffects of the creating resolution',
    { retry: TEST_RETRY, timeout: TEST_TIMEOUT },
    async ({ task, expect, onTestFinished }) => {
      const retryCount = task.result?.retryCount ?? 0;
      const { dir } = createTestWithMultiFiles('dev-rescan-side-effects', retryCount, {
        'main.js': `import './dep.js'\nimport { value } from './value.js'\nconsole.log(value)`,
        'dep.js': `console.log('EFFECT_1')`,
        'value.js': `export const value = 'VALUE_1'`,
      });
      const input = path.join(dir, 'main.js');
      const dep = path.join(dir, 'dep.js');

      const results: (string | Error)[] = [];
      let builds = 0;
      const engine = await dev(
        {
          input,
          plugins: [
            {
              name: 'test-plugin',
              buildStart() {
                builds++;
              },
              resolveId: (source) =>
                source === './dep.js' ? { id: dep, moduleSideEffects: false } : undefined,
              buildEnd() {
                // The cache keeps `dep.js` and fetches it again in the next build.
                if (builds === 2) throw new Error('planned failure');
              },
            },
          ],
        },
        { dir: path.join(dir, 'dist') },
        {
          rebuildStrategy: 'always',
          onOutput: (result) => {
            results.push(result instanceof Error ? result : chunkCode(result));
          },
        },
      );
      onTestFinished(async () => {
        await engine.close();
        if (!process.env.CI) {
          fs.rmSync(dir, { recursive: true, force: true });
        }
      });

      await engine.run();
      await expect.poll(() => results.length).toBe(1);
      expect(results[0]).not.toContain('EFFECT_1');

      await editFile(dep, `console.log('EFFECT_2')`);
      await expect.poll(() => results.length).toBe(2);
      expect(results[1]).toBeInstanceOf(Error);

      await editFile(path.join(dir, 'value.js'), `export const value = 'VALUE_2'`);
      await expect.poll(() => results.length).toBe(3);
      expect(builds).toBe(3);
      expect(results[2]).toContain('VALUE_2');
      expect(results[2]).not.toContain('EFFECT_2');
    },
  );

test
  .skipIf(isSingleThread)
  .concurrent(
    'a partial scan gives a changed module only the options of the resolution that created it',
    { retry: TEST_RETRY, timeout: TEST_TIMEOUT },
    async ({ task, expect, onTestFinished }) => {
      const retryCount = task.result?.retryCount ?? 0;
      const { dir } = createTestWithMultiFiles('dev-creator-options', retryCount, {
        'main.js': `import './x.js'\nimport './y.js'`,
        'x.js': `import './shared.js'`,
        'y.js': `import './shared.js'`,
        'shared.js': `console.log('EFFECT_1')`,
      });
      const input = path.join(dir, 'main.js');
      const shared = path.join(dir, 'shared.js');
      let markSharedCreated!: () => void;
      const sharedCreated = new Promise<void>((resolve) => {
        markSharedCreated = resolve;
      });

      const outputs: string[] = [];
      const seenByLoad: unknown[] = [];
      const engine = await dev(
        {
          input,
          plugins: [
            {
              name: 'test-plugin',
              async resolveId(source, importer, options) {
                if (source !== './shared.js') return;
                if (options.custom?.probe) {
                  return { id: shared, meta: { probe: true }, moduleSideEffects: false };
                }
                if (importer === path.join(dir, 'x.js')) {
                  return { id: shared, meta: { fromX: true } };
                }
                // Resolve the import in `y.js` only after the import in `x.js` created the module.
                await sharedCreated;
                return { id: shared, meta: { fromY: true }, moduleSideEffects: false };
              },
              load(id) {
                if (id !== shared) return;
                markSharedCreated();
                const { meta, moduleSideEffects } = this.getModuleInfo(shared)!;
                seenByLoad.push({ meta: { ...meta }, moduleSideEffects });
              },
              transform(code, id) {
                if (id === shared) return { code, meta: { transformed: true } };
              },
            },
            {
              name: 'prober',
              async buildEnd() {
                await this.resolve('./shared.js', input, { custom: { probe: true } });
              },
            },
          ],
        },
        { dir: path.join(dir, 'dist') },
        {
          rebuildStrategy: 'always',
          onOutput: (result) => {
            if (!(result instanceof Error)) outputs.push(chunkCode(result));
          },
        },
      );
      onTestFinished(async () => {
        await engine.close();
        if (!process.env.CI) {
          fs.rmSync(dir, { recursive: true, force: true });
        }
      });

      await engine.run();
      await expect.poll(() => outputs.length).toBe(1);
      expect.soft(outputs[0]).toContain('EFFECT_1');

      await editFile(shared, `console.log('EFFECT_2')`);
      await expect.poll(() => outputs.length).toBe(2);
      // The first build, the HMR stage and the rebuild.
      const creatorOptions = { meta: { fromX: true }, moduleSideEffects: true };
      expect.soft(seenByLoad).toEqual([creatorOptions, creatorOptions, creatorOptions]);
      expect.soft(outputs[1]).toContain('EFFECT_2');
    },
  );

test
  .skipIf(isSingleThread)
  .concurrent(
    'a module that a partial scan creates does not get the options of an earlier probe',
    { retry: TEST_RETRY, timeout: TEST_TIMEOUT },
    async ({ task, expect, onTestFinished }) => {
      const retryCount = task.result?.retryCount ?? 0;
      const { input, outputDir, dir } = createTestInputAndOutput('dev-probe-options', retryCount);
      const later = path.join(dir, 'later.js');
      fs.writeFileSync(input, `console.log('MAIN_1')`);
      fs.writeFileSync(later, `console.log('LATER_EFFECT')`);

      const outputs: string[] = [];
      const seenByLoad: unknown[] = [];
      const engine = await dev(
        {
          input,
          plugins: [
            {
              name: 'test-plugin',
              resolveId(source, _importer, options) {
                if (source !== './later.js') return;
                if (options.custom?.probe) {
                  return { id: later, meta: { probe: true }, moduleSideEffects: false };
                }
                return { id: later, meta: { imported: true } };
              },
              load(id) {
                if (id !== later) return;
                const { meta, moduleSideEffects } = this.getModuleInfo(later)!;
                seenByLoad.push({ meta: { ...meta }, moduleSideEffects });
              },
            },
            {
              name: 'prober',
              async transform(code, id) {
                if (id === input && code.includes('PROBE')) {
                  await this.resolve('./later.js', input, { custom: { probe: true } });
                }
              },
            },
          ],
        },
        { dir: outputDir },
        {
          rebuildStrategy: 'always',
          onOutput: (result) => {
            if (!(result instanceof Error)) outputs.push(chunkCode(result));
          },
        },
      );
      onTestFinished(async () => {
        await engine.close();
        if (!process.env.CI) {
          fs.rmSync(dir, { recursive: true, force: true });
        }
      });

      await engine.run();
      await expect.poll(() => outputs.length).toBe(1);

      // `later.js` is only probed, so it is not a module yet.
      await editFile(input, `// PROBE\nconsole.log('MAIN_2')`);
      await expect.poll(() => outputs.length).toBe(2);
      expect(seenByLoad).toEqual([]);

      await editFile(input, `import './later.js'\nconsole.log('MAIN_3')`);
      await expect.poll(() => outputs.length).toBe(3);
      expect(seenByLoad.at(-1)).toEqual({ meta: { imported: true }, moduleSideEffects: true });
      expect(outputs[2]).toContain('MAIN_3');
      expect(outputs[2]).toContain('LATER_EFFECT');
    },
  );

/** The number of saved resolution options in `PluginContextData`, which is internal. */
function savedResolutionOptions(pluginContext: object): number {
  const data = (pluginContext as { data: Record<string, Map<number, unknown> | undefined> }).data;
  // A renamed field must fail the test, not count as 0.
  if (!(data.resolutionOptionsMap instanceof Map)) throw new Error('no resolutionOptionsMap');
  return data.resolutionOptionsMap.size + (data.olderResolutionOptionsMap?.size ?? 0);
}

test
  .skipIf(isSingleThread)
  .concurrent(
    'an HMR-only session drops the options of resolutions that created no module',
    { retry: TEST_RETRY, timeout: TEST_TIMEOUT },
    async ({ task, expect, onTestFinished }) => {
      const retryCount = task.result?.retryCount ?? 0;
      const { dir } = createTestWithMultiFiles('dev-unused-resolutions', retryCount, {
        'main.js': `import './mid.js'`,
        'mid.js': `import './dep.js'\nconsole.log('MID_0')`,
        'dep.js': `console.log('DEP')`,
      });
      const input = path.join(dir, 'main.js');
      const mid = path.join(dir, 'mid.js');
      const dep = path.join(dir, 'dep.js');
      const EDITS = 5;

      const saved: number[] = [];
      const probeMeta: unknown[] = [];
      const onHmrUpdates = vi.fn();
      const engine = await dev(
        {
          input,
          plugins: [
            {
              name: 'test-plugin',
              // After the first build, `dep.js` exists, so each edit of `mid.js` saves options
              // that create no module.
              resolveId: (source) =>
                source === './dep.js' ? { id: dep, meta: { dep: true } } : null,
              async transform(_code, id) {
                if (id !== mid) return;
                saved.push(savedResolutionOptions(this));
                probeMeta.push((await this.resolve('./dep.js', mid, { skipSelf: false }))?.meta);
              },
            },
          ],
        },
        { dir: path.join(dir, 'dist') },
        { rebuildStrategy: 'never', onHmrUpdates },
      );
      onTestFinished(async () => {
        await engine.close();
        if (!process.env.CI) {
          fs.rmSync(dir, { recursive: true, force: true });
        }
      });

      await engine.run();
      await expect.poll(() => saved.length).toBe(1);
      for (let edit = 1; edit <= EDITS; edit++) {
        await editFile(mid, `import './dep.js'\nconsole.log('MID_${edit}')`);
        await expect.poll(() => onHmrUpdates.mock.calls.length).toBe(edit);
      }
      expect(saved).toHaveLength(EDITS + 1);
      // At most the options saved by the previous HMR update.
      expect(Math.max(...saved)).toBeLessThanOrEqual(1);
      expect(probeMeta).toEqual(Array(EDITS + 1).fill({ dep: true }));
    },
  );

test
  .skipIf(isSingleThread)
  .concurrent(
    'a moduleSideEffects write after buildEnd does not reach the next partial build',
    { retry: TEST_RETRY, timeout: TEST_TIMEOUT },
    async ({ task, expect, onTestFinished }) => {
      const retryCount = task.result?.retryCount ?? 0;
      const { input, outputDir, dir } = createTestInputAndOutput('dev-late-write', retryCount);
      const dep = path.join(dir, 'dep.js');
      fs.writeFileSync(input, `import './dep.js'\nconsole.log('MAIN_1')`);
      fs.writeFileSync(dep, `console.log('DEP_EFFECT')`);

      const outputs: string[] = [];
      let builds = 0;
      const engine = await dev(
        {
          input,
          logLevel: 'silent',
          plugins: [
            {
              name: 'late-writer',
              buildStart() {
                builds++;
              },
              renderStart() {
                if (builds === 1) this.getModuleInfo(dep)!.moduleSideEffects = false;
              },
            },
          ],
        },
        { dir: outputDir },
        {
          rebuildStrategy: 'always',
          onOutput: (result) => {
            if (!(result instanceof Error)) outputs.push(chunkCode(result));
          },
        },
      );
      onTestFinished(async () => {
        await engine.close();
        if (!process.env.CI) {
          fs.rmSync(dir, { recursive: true, force: true });
        }
      });

      await engine.run();
      await expect.poll(() => outputs.length).toBe(1);
      expect(outputs[0]).toContain('DEP_EFFECT');

      // Only main.js changes. A full build of the same sources keeps `dep.js`.
      await editFile(input, `import './dep.js'\nconsole.log('MAIN_2')`);
      await expect
        .poll(async () => {
          await engine.ensureLatestBuildOutput();
          return outputs.at(-1);
        })
        .toContain('MAIN_2');
      expect(outputs.at(-1)).toContain('DEP_EFFECT');
    },
  );
