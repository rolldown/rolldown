import { BindingBundler, shutdownAsyncRuntime, startAsyncRuntime } from '../binding.cjs';
import type { InputOptions } from '../options/input-options';
import { PluginDriver } from '../plugin/plugin-driver';
import { createBundlerOptions } from '../utils/create-bundler-option';
import { unwrapBindingResult } from '../utils/error';
import { validateOption } from '../utils/validator';
import { RolldownBuild } from './rolldown/rolldown-build';

export { freeExternalMemory } from '../types/external-memory-handle';

/**
 * This is an experimental API. Its behavior may change in the future.
 *
 * - Calling this API will only execute the `scan/build` stage of rolldown.
 * - `scan` will clean up all resources automatically, but if you want to ensure timely cleanup, you need to wait for the returned promise to resolve.
 *
 * @example To ensure cleanup of resources, use the returned promise to wait for the scan to complete.
 * ```ts
 * import { scan } from 'rolldown/api/experimental';
 *
 * const cleanupPromise = await scan(...);
 * await cleanupPromise;
 * // Now all resources have been cleaned up.
 * ```
 */
export const scan = async (
  rawInputOptions: InputOptions,
  rawOutputOptions = {},
): Promise<Promise<void>> => {
  validateOption('input', rawInputOptions);
  validateOption('output', rawOutputOptions);

  const inputOptions = await PluginDriver.callOptionsHook(rawInputOptions);
  const ret = await createBundlerOptions(inputOptions, rawOutputOptions, false);
  const bundler = new BindingBundler();

  if (RolldownBuild.asyncRuntimeShutdown) {
    startAsyncRuntime();
    RolldownBuild.asyncRuntimeShutdown = false;
  }

  let cleanupPromise: Promise<void> | undefined;
  const cleanup = () =>
    (cleanupPromise ??= (async () => {
      const errors: unknown[] = [];
      try {
        await bundler.close();
      } catch (error) {
        errors.push(error);
      }
      try {
        await ret.stopWorkers?.();
      } catch (error) {
        errors.push(error);
      }
      try {
        shutdownAsyncRuntime();
        RolldownBuild.asyncRuntimeShutdown = true;
      } catch (error) {
        errors.push(error);
      }
      if (errors.length === 1) throw errors[0];
      if (errors.length > 1) {
        throw new AggregateError(
          errors,
          'Scan native close, parallel-plugin worker shutdown, or runtime shutdown failed',
        );
      }
    })());

  try {
    const result = await bundler.scan(ret.bundlerOptions);
    unwrapBindingResult(result);
  } catch (error) {
    try {
      await cleanup();
    } catch (cleanupError) {
      throw new AggregateError([error, cleanupError], 'Scan and cleanup both failed');
    }
    throw error;
  }

  // Instead of blocking here, we return a promise to let the caller decide when to wait for cleanup.
  return cleanup();
};
