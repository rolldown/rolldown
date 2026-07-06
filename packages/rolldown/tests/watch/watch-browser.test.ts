import path from 'node:path';

import { chromium } from 'playwright-chromium';
import { rollup } from 'rollup';
import * as ts from 'typescript';
import { expect, test } from 'vitest';

const TEST_TIMEOUT = 30_000;
const browserTest = test.skipIf(process.env.ROLLDOWN_BROWSER_TEST !== '1');

browserTest(
  'browser watcher scheduling and close lifecycle',
  { timeout: TEST_TIMEOUT },
  async () => {
    const code = await buildBrowserWatcherHarness();
    const browser = await chromium.launch({ headless: true });
    try {
      const page = await browser.newPage();
      await page.addScriptTag({ content: code });
      const result = await page.evaluate(async () => {
        return (
          globalThis as typeof globalThis & {
            runBrowserWatcherTests(): Promise<{
              cancellation: Record<string, number | boolean>;
              cleanup: Record<string, number>;
              lifecycle: Record<string, number | boolean>;
              unsupported: Record<string, number | string | boolean | undefined>;
            }>;
          }
        ).runBrowserWatcherTests();
      });

      expect(result.lifecycle).toEqual({
        concurrentCloseDuringListenerSettled: true,
        initiatingCloseSettledBeforeListenerFinished: false,
        leaseReleaseCalls: 1,
        nativeCloseCalls: 1,
        reentrantCloseSettled: true,
        runCallsAfterHostTurn: 1,
        runCallsBeforeHostTurn: 0,
        stopWorkerCalls: 1,
      });
      expect(result.cancellation).toEqual({
        closeRejectedWithCancellationError: true,
        leaseReleaseCalls: 1,
        nativeCloseCalls: 1,
        runCalls: 0,
        stopWorkerCalls: 1,
      });
      expect(result.cleanup).toEqual({
        closeEvents: 1,
        leaseReleaseCalls: 3,
        nativeCloseCalls: 1,
        runCalls: 0,
        stopWorkerCalls: 3,
      });
      expect(result.unsupported).toEqual({
        bindingConstructions: 0,
        closeOvertookEnd: false,
        closeResolved: true,
        errorCloseResolved: true,
        errorCode: 'ERR_ROLLDOWN_UNSUPPORTED_RUNTIME_FEATURE',
        errorFeature: 'watch',
        events: 'ERROR,ERROR_CLOSE_RESOLVED,END,END_FINISHED,CLOSE,CLOSE_AFTER_END',
        leaseReleaseCalls: 0,
        optionsHookCalls: 0,
        stopWorkerCalls: 0,
      });
    } finally {
      await browser.close();
    }
  },
);

browserTest('browser parallel plugin capability and preflight contract', async () => {
  const code = await buildBrowserParallelPluginHarness();
  const browser = await chromium.launch({ headless: true });
  try {
    const page = await browser.newPage();
    await page.addScriptTag({ content: code });
    const result = await page.evaluate(async () => {
      return (
        globalThis as typeof globalThis & {
          runBrowserParallelPluginTest(): Promise<{
            descriptor: Record<string, number | string | boolean | undefined>;
            factory: Record<string, string | undefined>;
            ordinary: Record<string, number | boolean>;
          }>;
        }
      ).runBrowserParallelPluginTest();
    });

    expect(result).toEqual({
      descriptor: {
        bindingConstructions: 0,
        bindingifyCalls: 0,
        code: 'ERR_ROLLDOWN_UNSUPPORTED_RUNTIME_FEATURE',
        feature: 'parallelPlugins',
        leaseAcquisitions: 0,
        nativeGenerateCalls: 0,
        optionsHookCalls: 0,
        outputOptionsHookCalls: 0,
        pluginPromiseThenCalls: 0,
        registryConstructions: 0,
        rejected: true,
        timedOut: false,
      },
      factory: {
        code: 'ERR_ROLLDOWN_UNSUPPORTED_RUNTIME_FEATURE',
        feature: 'parallelPlugins',
        name: 'UnsupportedRuntimeFeatureError',
        runtimeTarget: 'wasi',
      },
      ordinary: {
        bindingConstructions: 1,
        nativeGenerateCalls: 1,
        optionsHookCalls: 1,
        succeeded: true,
      },
    });
  } finally {
    await browser.close();
  }
});

async function buildBrowserWatcherHarness(): Promise<string> {
  const watchIndexPath = path.resolve(import.meta.dirname, '../../src/api/watch/index.ts');
  const watcherPath = path.resolve(import.meta.dirname, '../../src/api/watch/watcher.ts');
  const emitterPath = path.resolve(import.meta.dirname, '../../src/api/watch/watch-emitter.ts');
  const runtimeSupportPath = path.resolve(import.meta.dirname, '../../src/runtime-support.ts');
  const asyncContextPath = path.resolve(import.meta.dirname, '../../src/utils/async-context.ts');
  const retryableCleanupPath = path.resolve(
    import.meta.dirname,
    '../../src/utils/retryable-cleanup.ts',
  );
  const virtualModules = new Map<string, string>([
    [
      'binding',
      `
        export class BindingWatcher {
          constructor(_options, callback) {
            this.callback = callback;
            globalThis.__watchHarness.bindingConstructed += 1;
          }
          async run() {
            globalThis.__watchHarness.runCalls += 1;
          }
          waitForClose() {
            globalThis.__watchHarness.waitForCloseCalls += 1;
          }
          async close() {
            globalThis.__watchHarness.nativeCloseCalls += 1;
            await Promise.resolve();
            await this.callback({ eventKind: () => 'close' });
          }
        }
        export function getRuntimeCapabilities() {
          const harness = globalThis.__watchHarness;
          return {
            devSupported: true,
            flavor: 'MultiThread',
            target: harness?.watchSupported === false ? 'wasi' : 'native',
            threads: true,
            wasi: harness?.watchSupported === false,
            watchSupported: harness?.watchSupported !== false,
          };
        }
      `,
    ],
    [
      'runtime-lifecycle',
      `
        export class CloseCoordinator {
          constructor(message) {
            this.message = message;
          }
          close(attempt) {
            return (this.promise ??= this.run(attempt));
          }
          async run(attempt) {
            const result = await attempt();
            if (result.retryable) this.promise = undefined;
            if (result.errors.length === 1) throw result.errors[0];
            if (result.errors.length > 1) {
              throw new AggregateError(result.errors, this.message);
            }
          }
        }

        export function acquireRuntimeLease() {
          const harness = globalThis.__watchHarness;
          let released = false;
          return {
            release() {
              if (released) return;
              harness.leaseReleaseCalls += 1;
              if (harness.leaseReleaseFailures > 0) {
                harness.leaseReleaseFailures -= 1;
                throw harness.leaseReleaseError;
              }
              released = true;
            },
          };
        }
      `,
    ],
    [
      'create-bundler-option',
      `
        export async function createBundlerOptions(inputOptions) {
          const harness = globalThis.__watchHarness;
          let stopped = false;
          return {
            bundlerOptions: {},
            inputOptions: { ...inputOptions, watch: inputOptions.watch ?? null },
            onLog() {},
            async stopWorkers() {
              if (stopped) return;
              harness.stopWorkerCalls += 1;
              if (harness.stopWorkerFailures > 0) {
                harness.stopWorkerFailures -= 1;
                throw harness.stopWorkerError;
              }
              stopped = true;
            },
          };
        }
      `,
    ],
    [
      'plugin-driver',
      `
        export const PluginDriver = {
          async callOptionsHook(option) {
            globalThis.__watchHarness.optionsHookCalls += 1;
            return option;
          },
        };
      `,
    ],
    ['logging', `export const LOG_LEVEL_WARN = 'warn';`],
    ['logs', `export function logMultipleWatcherOption() { return {}; }`],
    ['error', `export function aggregateBindingErrorsIntoJsError(error) { return error; }`],
    ['misc', `export function arraify(value) { return Array.isArray(value) ? value : [value]; }`],
    ['async-hooks', `export class AsyncLocalStorage {}`],
  ]);

  const bundle = await rollup({
    input: 'browser-watcher-harness',
    plugins: [
      {
        name: 'browser-watcher-harness',
        resolveId(id, importer) {
          if (id === 'browser-watcher-harness') return `\0${id}`;
          if (id === 'node:async_hooks') return '\0async-hooks';
          if (!importer) return;
          if (id === './watcher') return watcherPath;
          if (id === './watch-emitter') return emitterPath;
          if (id === '../../runtime-support') return runtimeSupportPath;
          if (id === './binding.cjs') return '\0binding';
          if (id === '../../utils/async-context') return asyncContextPath;
          if (id === '../../binding.cjs') return '\0binding';
          if (id === '../../runtime-lifecycle') return '\0runtime-lifecycle';
          if (id === '../../utils/create-bundler-option') return '\0create-bundler-option';
          if (id === '../../utils/retryable-cleanup') return retryableCleanupPath;
          if (id === '../../plugin/plugin-driver') return '\0plugin-driver';
          if (id === '../../log/logging') return '\0logging';
          if (id === '../../log/logs') return '\0logs';
          if (id === '../../utils/error') return '\0error';
          if (id === '../../utils/misc') return '\0misc';
        },
        load(id) {
          if (id === '\0browser-watcher-harness') {
            return browserHarnessEntry(watchIndexPath, watcherPath, emitterPath);
          }
          return virtualModules.get(id.slice(1));
        },
      },
      {
        name: 'transpile-browser-watcher-harness',
        transform(code, id) {
          if (!id.endsWith('.ts')) return;
          return {
            code: ts.transpileModule(code.replaceAll('import.meta.browserBuild', 'true'), {
              compilerOptions: {
                module: ts.ModuleKind.ESNext,
                target: ts.ScriptTarget.ES2022,
              },
              fileName: id,
            }).outputText,
            map: null,
          };
        },
      },
    ],
  });

  try {
    const output = await bundle.generate({ format: 'iife', name: 'BrowserWatcherHarness' });
    return output.output.find((item) => item.type === 'chunk')!.code;
  } finally {
    await bundle.close();
  }
}

async function buildBrowserParallelPluginHarness(): Promise<string> {
  const buildPath = path.resolve(import.meta.dirname, '../../src/api/build.ts');
  const rolldownPath = path.resolve(import.meta.dirname, '../../src/api/rolldown/index.ts');
  const rolldownBuildPath = path.resolve(
    import.meta.dirname,
    '../../src/api/rolldown/rolldown-build.ts',
  );
  const createBundlerOptionPath = path.resolve(
    import.meta.dirname,
    '../../src/utils/create-bundler-option.ts',
  );
  const parallelPluginPath = path.resolve(
    import.meta.dirname,
    '../../src/plugin/parallel-plugin.ts',
  );
  const runtimeSupportPath = path.resolve(import.meta.dirname, '../../src/runtime-support.ts');
  const virtualModules = new Map<string, string>([
    [
      'binding',
      `
        export class BindingBundler {
          constructor() {
            globalThis.__parallelPluginHarness.bindingConstructions += 1;
            this.closed = false;
          }
          async generate() {
            globalThis.__parallelPluginHarness.nativeGenerateCalls += 1;
            return {};
          }
          async close() {
            this.closed = true;
          }
          getWatchFiles() {
            return [];
          }
        }
        export function getRuntimeCapabilities() {
          return {
            devSupported: true,
            flavor: 'MultiThread',
            target: 'wasi',
            threads: true,
            wasi: true,
            watchSupported: false,
          };
        }
      `,
    ],
    [
      'runtime-lifecycle',
      `
        export class CloseCoordinator {
          constructor(message) {
            this.message = message;
          }
          close(attempt) {
            return (this.promise ??= this.run(attempt));
          }
          async run(attempt) {
            const result = await attempt();
            if (result.retryable) this.promise = undefined;
            if (result.errors.length === 1) throw result.errors[0];
            if (result.errors.length > 1) {
              throw new AggregateError(result.errors, this.message);
            }
          }
        }
        export async function acquireRuntimeLease() {
          globalThis.__parallelPluginHarness.leaseAcquisitions += 1;
          return { release() {} };
        }
      `,
    ],
    [
      'plugin-driver',
      `
        export const PluginDriver = {
          async callOptionsHook(options) {
            const plugins = (await Promise.all(options.plugins ?? [])).flat(Infinity).filter(Boolean);
            for (const plugin of plugins) {
              if (plugin.options) {
                globalThis.__parallelPluginHarness.optionsHookCalls += 1;
                options = (await plugin.options(options)) || options;
              }
            }
            return options;
          },
          callOutputOptionsHook(plugins, options) {
            for (const plugin of plugins) {
              if (plugin.outputOptions) {
                globalThis.__parallelPluginHarness.outputOptionsHookCalls += 1;
                options = plugin.outputOptions(options) || options;
              }
            }
            return options;
          },
        };
        export function getObjectPlugins(plugins) {
          return plugins.filter((plugin) => plugin && !('_parallel' in plugin));
        }
      `,
    ],
    [
      'normalize-plugin-option',
      `
        export const ANONYMOUS_OUTPUT_PLUGIN_PREFIX = 'output';
        export const ANONYMOUS_PLUGIN_PREFIX = 'input';
        export function checkOutputPluginOption(plugins) {
          return plugins;
        }
        export async function normalizePluginOption(value) {
          let plugins = [value];
          do {
            plugins = (await Promise.all(plugins)).flat(Infinity);
          } while (plugins.some((plugin) => plugin?.then));
          return plugins.filter(Boolean);
        }
        export function normalizePlugins(plugins) {
          return plugins;
        }
      `,
    ],
    [
      'logger',
      `
        export function getLogger() {
          return () => {};
        }
        export function getOnLog() {
          return () => {};
        }
      `,
    ],
    ['logging', `export const LOG_LEVEL_INFO = 'info';`],
    ['plugin-context-data', `export class PluginContextData {}`],
    [
      'bindingify-input-options',
      `
        export function bindingifyInputOptions() {
          globalThis.__parallelPluginHarness.bindingifyCalls += 1;
          return {};
        }
      `,
    ],
    ['bindingify-output-options', `export function bindingifyOutputOptions() { return {}; }`],
    [
      'initialize-parallel-plugins',
      `
        export async function initializeParallelPlugins() {
          globalThis.__parallelPluginHarness.registryConstructions += 1;
          throw new Error('browser build initialized parallel workers');
        }
      `,
    ],
    [
      'retryable-cleanup',
      `
        export function createCleanupFailureError(error) {
          return error;
        }
        export function isCleanupFailureError() {
          return false;
        }
        export function runRetryableCleanup(cleanup) {
          return cleanup();
        }
        export async function retryCleanupFromError(error) {
          throw error;
        }
        export function trackRetryableCleanupOwnership() {}
      `,
    ],
    ['validator', `export function validateOption() {}`],
    ['error', `export function unwrapBindingResult(value) { return value; }`],
    ['rolldown-output-impl', `export class RolldownOutputImpl {}`],
    ['node-url', `export function pathToFileURL(value) { return { href: String(value) }; }`],
  ]);

  const bundle = await rollup({
    input: 'browser-parallel-plugin-harness',
    plugins: [
      {
        name: 'browser-parallel-plugin-harness',
        resolveId(id, importer) {
          if (id === 'browser-parallel-plugin-harness') return `\0${id}`;
          if (id === 'node:url') return '\0node-url';
          if (!importer) return;
          if (id === './rolldown-build') return rolldownBuildPath;
          if (id === './rolldown') return rolldownPath;
          if (id === './rolldown/rolldown-build') return rolldownBuildPath;
          if (id === '../../binding.cjs' || id === '../binding.cjs' || id === './binding.cjs') {
            return '\0binding';
          }
          if (id === '../../runtime-lifecycle') return '\0runtime-lifecycle';
          if (id === '../runtime-support') return runtimeSupportPath;
          if (id === '../../plugin/plugin-driver' || id === '../plugin/plugin-driver') {
            return '\0plugin-driver';
          }
          if (id === '../../utils/validator') return '\0validator';
          if (id === '../../types/rolldown-output-impl') return '\0rolldown-output-impl';
          if (id === '../../utils/error') return '\0error';
          if (id === '../../utils/create-bundler-option') return createBundlerOptionPath;
          if (id === '../log/logger') return '\0logger';
          if (id === '../log/logging') return '\0logging';
          if (id === '../plugin/plugin-context-data') return '\0plugin-context-data';
          if (id === '../plugin/parallel-plugin' || id === '../../plugin/parallel-plugin') {
            return parallelPluginPath;
          }
          if (id === './bindingify-input-options') return '\0bindingify-input-options';
          if (id === './bindingify-output-options') return '\0bindingify-output-options';
          if (id === './initialize-parallel-plugins') return '\0initialize-parallel-plugins';
          if (id === './retryable-cleanup' || id === '../utils/retryable-cleanup') {
            return '\0retryable-cleanup';
          }
          if (id === './normalize-plugin-option') return '\0normalize-plugin-option';
        },
        load(id) {
          if (id === '\0browser-parallel-plugin-harness') {
            return browserParallelPluginHarnessEntry(buildPath, parallelPluginPath);
          }
          return virtualModules.get(id.slice(1));
        },
      },
      {
        name: 'transpile-browser-parallel-plugin-harness',
        transform(code, id) {
          if (!id.endsWith('.ts')) return;
          return {
            code: ts.transpileModule(code.replaceAll('import.meta.browserBuild', 'true'), {
              compilerOptions: {
                module: ts.ModuleKind.ESNext,
                target: ts.ScriptTarget.ES2022,
              },
              fileName: id,
            }).outputText,
            map: null,
          };
        },
      },
    ],
  });

  try {
    const output = await bundle.generate({ format: 'iife', name: 'BrowserParallelPluginHarness' });
    return output.output.find((item) => item.type === 'chunk')!.code;
  } finally {
    await bundle.close();
  }
}

function browserHarnessEntry(
  watchIndexPath: string,
  watcherPath: string,
  emitterPath: string,
): string {
  return `
    import { watch } from ${JSON.stringify(watchIndexPath)};
    import { createWatcher } from ${JSON.stringify(watcherPath)};
    import { WatcherEmitter } from ${JSON.stringify(emitterPath)};

    function resetHarness() {
      globalThis.__watchHarness = {
        bindingConstructed: 0,
        leaseReleaseCalls: 0,
        leaseReleaseError: new Error('lease release failed'),
        leaseReleaseFailures: 0,
        nativeCloseCalls: 0,
        optionsHookCalls: 0,
        runCalls: 0,
        stopWorkerCalls: 0,
        stopWorkerError: new Error('worker stop failed'),
        stopWorkerFailures: 0,
        waitForCloseCalls: 0,
        watchSupported: true,
      };
      return globalThis.__watchHarness;
    }

    async function withTimeout(callback) {
      return Promise.race([
        callback(),
        new Promise((_, reject) => {
          globalThis.setTimeout(
            () => reject(new Error('browser watcher regression timed out')),
            5_000,
          );
        }),
      ]);
    }

    globalThis.runBrowserWatcherTests = () => withTimeout(async () => {
      const lifecycleHarness = resetHarness();
      const emitter = new WatcherEmitter();
      await createWatcher(emitter, { output: {} });
      const runCallsBeforeHostTurn = lifecycleHarness.runCalls;
      await new Promise((resolve) => globalThis.setTimeout(resolve, 0));
      const runCallsAfterHostTurn = lifecycleHarness.runCalls;

      let releaseCloseListener;
      const closeListenerGate = new Promise((resolve) => {
        releaseCloseListener = resolve;
      });
      let markReentrantCloseSettled;
      const reentrantCloseSettled = new Promise((resolve) => {
        markReentrantCloseSettled = resolve;
      });
      emitter.on('close', async () => {
        await Promise.resolve();
        await emitter.close();
        markReentrantCloseSettled();
        await closeListenerGate;
      });

      let initiatingCloseSettled = false;
      const firstClose = emitter.close().finally(() => {
        initiatingCloseSettled = true;
      });
      await reentrantCloseSettled;
      let concurrentCloseDuringListenerSettled = false;
      const concurrentCloseDuringListener = emitter.close().finally(() => {
        concurrentCloseDuringListenerSettled = true;
      });
      await Promise.resolve();
      await new Promise((resolve) => globalThis.setTimeout(resolve, 0));
      const initiatingCloseSettledBeforeListenerFinished = initiatingCloseSettled;
      releaseCloseListener();
      await Promise.all([firstClose, concurrentCloseDuringListener]);

      const lifecycle = {
        concurrentCloseDuringListenerSettled,
        initiatingCloseSettledBeforeListenerFinished,
        leaseReleaseCalls: lifecycleHarness.leaseReleaseCalls,
        nativeCloseCalls: lifecycleHarness.nativeCloseCalls,
        reentrantCloseSettled: true,
        runCallsAfterHostTurn,
        runCallsBeforeHostTurn,
        stopWorkerCalls: lifecycleHarness.stopWorkerCalls,
      };

      const cancellationHarness = resetHarness();
      const cancellationEmitter = new WatcherEmitter();
      await createWatcher(cancellationEmitter, { output: {} });
      const cancellationError = new Error('host turn cancellation failed');
      const originalClearTimeout = globalThis.clearTimeout;
      globalThis.clearTimeout = () => {
        throw cancellationError;
      };
      const cancellationResult = await Promise.allSettled([cancellationEmitter.close()]);
      globalThis.clearTimeout = originalClearTimeout;
      await new Promise((resolve) => globalThis.setTimeout(resolve, 0));
      const cancellation = {
        closeRejectedWithCancellationError:
          cancellationResult[0].status === 'rejected' &&
          cancellationResult[0].reason === cancellationError,
        leaseReleaseCalls: cancellationHarness.leaseReleaseCalls,
        nativeCloseCalls: cancellationHarness.nativeCloseCalls,
        runCalls: cancellationHarness.runCalls,
        stopWorkerCalls: cancellationHarness.stopWorkerCalls,
      };

      const cleanupHarness = resetHarness();
      cleanupHarness.leaseReleaseFailures = 2;
      cleanupHarness.stopWorkerFailures = 2;
      const cleanupEmitter = new WatcherEmitter();
      let cleanupCloseEvents = 0;
      cleanupEmitter.on('close', () => {
        cleanupCloseEvents += 1;
      });
      const originalSetTimeout = globalThis.setTimeout;
      globalThis.setTimeout = () => {
        throw new Error('host turn scheduling failed');
      };
      let cleanupSetupError;
      try {
        await createWatcher(cleanupEmitter, { output: {} });
        throw new Error('watcher creation unexpectedly succeeded');
      } catch (error) {
        const errors = error instanceof AggregateError ? error.errors : [error];
        if (!errors.some((item) => String(item).includes('host turn scheduling failed'))) {
          throw error;
        }
        cleanupSetupError = error;
      } finally {
        globalThis.setTimeout = originalSetTimeout;
      }
      await cleanupEmitter.failSetup(cleanupSetupError);
      await cleanupEmitter.close();

      const unsupportedHarness = resetHarness();
      unsupportedHarness.watchSupported = false;
      const unsupportedWatcher = watch({ output: {} });
      const unsupportedEvents = [];
      let unsupportedError;
      let releaseUnsupportedEnd;
      const unsupportedEndGate = new Promise((resolve) => {
        releaseUnsupportedEnd = resolve;
      });
      let markUnsupportedEndStarted;
      const unsupportedEndStarted = new Promise((resolve) => {
        markUnsupportedEndStarted = resolve;
      });
      let markUnsupportedEndObserved;
      const unsupportedEndObserved = new Promise((resolve) => {
        markUnsupportedEndObserved = resolve;
      });
      let errorCloseResolved = false;
      unsupportedWatcher.on('event', async (event) => {
          unsupportedEvents.push(event.code);
          if (event.code === 'ERROR') {
            unsupportedError = event.error;
            await unsupportedWatcher.close();
            errorCloseResolved = true;
            unsupportedEvents.push('ERROR_CLOSE_RESOLVED');
          } else if (event.code === 'END') {
            markUnsupportedEndStarted();
            await unsupportedEndGate;
            unsupportedEvents.push('END_FINISHED');
            markUnsupportedEndObserved();
          }
      });
      unsupportedWatcher.on('close', async () => {
        unsupportedEvents.push('CLOSE');
        await unsupportedEndObserved;
        unsupportedEvents.push('CLOSE_AFTER_END');
      });
      let unsupportedCloseSettled = false;
      const unsupportedClose = unsupportedWatcher.close().finally(() => {
        unsupportedCloseSettled = true;
      });
      await unsupportedEndStarted;
      await Promise.resolve();
      const closeOvertookEnd = unsupportedCloseSettled;
      releaseUnsupportedEnd();
      await unsupportedClose;
      await unsupportedWatcher.close();

      return {
        cancellation,
        cleanup: {
          closeEvents: cleanupCloseEvents,
          leaseReleaseCalls: cleanupHarness.leaseReleaseCalls,
          nativeCloseCalls: cleanupHarness.nativeCloseCalls,
          runCalls: cleanupHarness.runCalls,
          stopWorkerCalls: cleanupHarness.stopWorkerCalls,
        },
        lifecycle,
        unsupported: {
          bindingConstructions: unsupportedHarness.bindingConstructed,
          closeOvertookEnd,
          closeResolved: true,
          errorCloseResolved,
          errorCode: unsupportedError?.code,
          errorFeature: unsupportedError?.feature,
          events: unsupportedEvents.join(','),
          leaseReleaseCalls: unsupportedHarness.leaseReleaseCalls,
          optionsHookCalls: unsupportedHarness.optionsHookCalls,
          stopWorkerCalls: unsupportedHarness.stopWorkerCalls,
        },
      };
    });
  `;
}

function browserParallelPluginHarnessEntry(buildPath: string, parallelPluginPath: string): string {
  return `
    import { build } from ${JSON.stringify(buildPath)};
    import { defineParallelPlugin } from ${JSON.stringify(parallelPluginPath)};

    globalThis.runBrowserParallelPluginTest = async () => {
      globalThis.__parallelPluginHarness = {
        bindingConstructions: 0,
        bindingifyCalls: 0,
        leaseAcquisitions: 0,
        nativeGenerateCalls: 0,
        optionsHookCalls: 0,
        outputOptionsHookCalls: 0,
        pluginPromiseThenCalls: 0,
        registryConstructions: 0,
      };

      let factoryError;
      try {
        defineParallelPlugin('/project/plugin.mjs');
      } catch (caught) {
        factoryError = caught;
      }

      const hangingPlugin = {
        then() {
          globalThis.__parallelPluginHarness.pluginPromiseThenCalls += 1;
          return new Promise(() => {});
        },
      };
      let descriptorError;
      let timedOut = false;
      try {
        await Promise.race([
          build({
            plugins: [
              {
                name: 'input-options-side-effect',
                options(options) {
                  return options;
                },
              },
              hangingPlugin,
            ],
            output: {
              plugins: [
                {
                  name: 'output-options-side-effect',
                  outputOptions(options) {
                    return options;
                  },
                },
                {
                  _parallel: {
                    fileUrl: 'file:///project/old-package-plugin.mjs',
                    options: {},
                  },
                },
              ],
            },
            write: false,
          }),
          new Promise((_, reject) => {
            setTimeout(() => {
              timedOut = true;
              reject(new Error('parallel descriptor preflight timed out'));
            }, 2_000);
          }),
        ]);
      } catch (caught) {
        descriptorError = caught;
      }

      const descriptor = {
        bindingConstructions: globalThis.__parallelPluginHarness.bindingConstructions,
        bindingifyCalls: globalThis.__parallelPluginHarness.bindingifyCalls,
        code: descriptorError?.code,
        feature: descriptorError?.feature,
        leaseAcquisitions: globalThis.__parallelPluginHarness.leaseAcquisitions,
        nativeGenerateCalls: globalThis.__parallelPluginHarness.nativeGenerateCalls,
        optionsHookCalls: globalThis.__parallelPluginHarness.optionsHookCalls,
        outputOptionsHookCalls: globalThis.__parallelPluginHarness.outputOptionsHookCalls,
        pluginPromiseThenCalls: globalThis.__parallelPluginHarness.pluginPromiseThenCalls,
        registryConstructions: globalThis.__parallelPluginHarness.registryConstructions,
        rejected: descriptorError !== undefined,
        timedOut,
      };

      globalThis.__parallelPluginHarness.bindingConstructions = 0;
      globalThis.__parallelPluginHarness.nativeGenerateCalls = 0;
      globalThis.__parallelPluginHarness.optionsHookCalls = 0;
      await build({
        plugins: [{
          name: 'ordinary-object-plugin',
          options(options) {
            return options;
          },
        }],
        write: false,
      });

      return {
        descriptor,
        factory: {
          code: factoryError?.code,
          feature: factoryError?.feature,
          name: factoryError?.name,
          runtimeTarget: factoryError?.runtime?.target,
        },
        ordinary: {
          bindingConstructions: globalThis.__parallelPluginHarness.bindingConstructions,
          nativeGenerateCalls: globalThis.__parallelPluginHarness.nativeGenerateCalls,
          optionsHookCalls: globalThis.__parallelPluginHarness.optionsHookCalls,
          succeeded: true,
        },
      };
    };
  `;
}
