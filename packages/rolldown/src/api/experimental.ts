import { BindingBundler } from '../binding.cjs';
import type { InputOptions } from '../options/input-options';
import { PluginDriver } from '../plugin/plugin-driver';
import { acquireRuntimeLease, type RuntimeLease } from '../runtime-lifecycle';
import { createBundlerOptions } from '../utils/create-bundler-option';
import { unwrapBindingResult } from '../utils/error';
import { validateOption } from '../utils/validator';

export { freeExternalMemory } from '../types/external-memory-handle';

/**
 * This is an experimental API. Its behavior may change in the future.
 *
 * - Calling this API will only execute the `scan/build` stage of rolldown.
 * - `scan` waits for all resources to be cleaned up before its promise resolves.
 *
 * @example Wait for the scan and its cleanup to complete.
 * ```ts
 * import { scan } from 'rolldown/api/experimental';
 *
 * await scan(...);
 * // Now all resources have been cleaned up.
 * ```
 */
export const scan = async (rawInputOptions: InputOptions, rawOutputOptions = {}): Promise<void> => {
  validateOption('input', rawInputOptions);
  validateOption('output', rawOutputOptions);

  const inputOptions = await PluginDriver.callOptionsHook(rawInputOptions);
  const ret = await createBundlerOptions(inputOptions, rawOutputOptions, false);
  let runtimeLease: RuntimeLease;
  try {
    runtimeLease = acquireRuntimeLease();
  } catch (error) {
    try {
      await ret.stopWorkers?.();
    } catch (cleanupError) {
      throw new AggregateError(
        [error, cleanupError],
        'Scan runtime setup and parallel-plugin worker cleanup both failed',
      );
    }
    throw error;
  }

  let bundler: BindingBundler;
  try {
    bundler = new BindingBundler();
  } catch (error) {
    const errors = [error];
    try {
      await ret.stopWorkers?.();
    } catch (cleanupError) {
      errors.push(cleanupError);
    }
    try {
      runtimeLease.release();
    } catch (cleanupError) {
      errors.push(cleanupError);
    }
    throw errors.length === 1 ? error : new AggregateError(errors, 'Scan setup and cleanup failed');
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
        runtimeLease.release();
      } catch (error) {
        errors.push(error);
      }
      if (errors.length === 1) throw errors[0];
      if (errors.length > 1) {
        throw new AggregateError(
          errors,
          'Scan native close, parallel-plugin worker shutdown, or runtime release failed',
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

  await cleanup();
};
