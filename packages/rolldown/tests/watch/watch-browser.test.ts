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
              transport: Record<string, number | boolean>;
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
      expect(result.transport).toEqual({
        closeRejectedWithNativeError: true,
        leaseReleaseCallsAfterFailure: 0,
        leaseReleaseCalls: 1,
        nativeCloseCalls: 2,
        retryResolved: true,
        stopWorkerCallsAfterFailure: 0,
        stopWorkerCalls: 1,
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

async function buildBrowserWatcherHarness(): Promise<string> {
  const watchIndexPath = path.resolve(import.meta.dirname, '../../src/api/watch/index.ts');
  const watcherPath = path.resolve(import.meta.dirname, '../../src/api/watch/watcher.ts');
  const emitterPath = path.resolve(import.meta.dirname, '../../src/api/watch/watch-emitter.ts');
  const runtimeSupportPath = path.resolve(import.meta.dirname, '../../src/runtime-support.ts');
  const bindingMismatchErrorPath = path.resolve(
    import.meta.dirname,
    '../../src/utils/binding-mismatch-error.ts',
  );
  const asyncContextPath = path.resolve(import.meta.dirname, '../../src/utils/async-context.ts');
  const prototypeChainPath = path.resolve(
    import.meta.dirname,
    '../../src/utils/prototype-chain.ts',
  );
  const closeCallbackScopePath = path.resolve(
    import.meta.dirname,
    '../../src/utils/close-callback-scope.ts',
  );
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
            if (globalThis.__watchHarness.nativeCloseError) {
              throw globalThis.__watchHarness.nativeCloseError;
            }
            await this.callback({ eventKind: () => 'close' });
            return { errors: [], nativeOwnedCloseIdentities: [] };
          }
        }
        export function getRuntimeCapabilities() {
          const harness = globalThis.__watchHarness;
          const wasi = harness?.watchSupported === false;
          return {
            asyncRuntimeBuild: false,
            backend: 'tokio',
            blockOnJsThreadSafe: false,
            devSupported: !wasi,
            flavor: wasi ? 'CurrentThread' : 'MultiThread',
            target: wasi ? 'wasi' : 'native',
            threads: !wasi,
            timers: !wasi,
            wasi,
            watchSupported: !wasi,
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
            return (this.promise ??= Promise.resolve().then(() => this.run(attempt)));
          }
          async run(attempt) {
            const result = await attempt();
            if (result.retryable) this.promise = undefined;
            throwCloseErrors(result.errors, this.message);
          }
        }

        export function throwCloseErrors(errors, message) {
          if (errors.length === 1) throw errors[0];
          if (errors.length > 1) {
            throw new AggregateError(errors, message, { cause: errors[0] });
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
            releaseOptionBoxes() {},
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
    [
      'error',
      `
        export function aggregateBindingErrorsIntoJsError(error) { return error; }
        export function normalizeBindingError(error) { return error; }
      `,
    ],
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
          if (id === './utils/binding-mismatch-error') return bindingMismatchErrorPath;
          if (id === './binding.cjs') return '\0binding';
          if (id === '../../utils/async-context' || id === './async-context') {
            return asyncContextPath;
          }
          if (importer === asyncContextPath && id === './prototype-chain') {
            return prototypeChainPath;
          }
          if (id === '../../utils/close-callback-scope') return closeCallbackScopePath;
          if (id === '../../binding.cjs') return '\0binding';
          if (id === '../../runtime-lifecycle' || id === '../runtime-lifecycle') {
            return '\0runtime-lifecycle';
          }
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
            return browserHarnessEntry(watchIndexPath, watcherPath, emitterPath, asyncContextPath);
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

function browserHarnessEntry(
  watchIndexPath: string,
  watcherPath: string,
  emitterPath: string,
  asyncContextPath: string,
): string {
  return `
    import { watch } from ${JSON.stringify(watchIndexPath)};
    import { createWatcher } from ${JSON.stringify(watcherPath)};
    import { WatcherEmitter } from ${JSON.stringify(emitterPath)};
    import { configureAsyncContext } from ${JSON.stringify(asyncContextPath)};

    function resetHarness() {
      globalThis.__watchHarness = {
        bindingConstructed: 0,
        leaseReleaseCalls: 0,
        leaseReleaseError: new Error('lease release failed'),
        leaseReleaseFailures: 0,
        nativeCloseCalls: 0,
        nativeCloseError: undefined,
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

    globalThis.runBrowserWatcherLateProviderTest = (createStorage) => withTimeout(async () => {
      const harness = resetHarness();
      let storageCreations = 0;
      configureAsyncContext({
        createStorage() {
          storageCreations += 1;
          return createStorage();
        },
      });

      const emitter = new WatcherEmitter();
      const storageCreationsAfterConstruction = storageCreations;
      await createWatcher(emitter, { output: {} });
      const storageCreationsBeforeClose = storageCreations;
      let reentrantClose;
      emitter.on('close', async () => {
        await Promise.resolve();
        reentrantClose = emitter.close();
      });
      await emitter.close();
      await reentrantClose;
      return {
        reentrantCloseSettled: true,
        storageCreations,
        storageCreationsAfterConstruction,
        storageCreationsBeforeClose,
      };
    });

    globalThis.runBrowserWatcherProviderRetryTest = (createStorage) => withTimeout(async () => {
      let listenerCalls = 0;
      const listener = async () => {
        listenerCalls += 1;
        await Promise.resolve();
      };

      const unavailableEmitter = new WatcherEmitter();
      unavailableEmitter.on('close', listener);
      await unavailableEmitter.emitClose(Promise.resolve());

      let storageCreations = 0;
      configureAsyncContext({
        createStorage() {
          storageCreations += 1;
          return createStorage();
        },
      });
      const configuredEmitter = new WatcherEmitter();
      configuredEmitter.on('close', listener);
      await configuredEmitter.emitClose(Promise.resolve());

      return {
        listenerCalls,
        storageCreations,
      };
    });

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

      const transportHarness = resetHarness();
      const transportEmitter = new WatcherEmitter();
      await createWatcher(transportEmitter, { output: {} });
      const nativeCloseError = new Error('native close transport failed');
      transportHarness.nativeCloseError = nativeCloseError;
      const transportResult = await Promise.allSettled([transportEmitter.close()]);
      const leaseReleaseCallsAfterFailure = transportHarness.leaseReleaseCalls;
      const stopWorkerCallsAfterFailure = transportHarness.stopWorkerCalls;
      transportHarness.nativeCloseError = undefined;
      const retryResult = await Promise.allSettled([transportEmitter.close()]);
      const transport = {
        closeRejectedWithNativeError:
          transportResult[0].status === 'rejected' &&
          transportResult[0].reason === nativeCloseError,
        leaseReleaseCallsAfterFailure,
        leaseReleaseCalls: transportHarness.leaseReleaseCalls,
        nativeCloseCalls: transportHarness.nativeCloseCalls,
        retryResolved: retryResult[0].status === 'fulfilled',
        stopWorkerCallsAfterFailure,
        stopWorkerCalls: transportHarness.stopWorkerCalls,
      };

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
        transport,
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
