import { BindingBundler } from '../binding.cjs';
import type { InputOptions } from '../options/input-options';
import type { OutputOptions } from '../options/output-options';
import { assertParallelPluginOptionsSupported } from '../plugin/parallel-plugin';
import { PluginDriver } from '../plugin/plugin-driver';
import { acquireRuntimeLease, type RuntimeLease } from '../runtime-lifecycle';
import { createBundlerOptions } from '../utils/create-bundler-option';
import { unwrapBindingResult } from '../utils/error';
import {
  attachRetryableCleanup,
  createCleanupFailureError,
  getRetryableCleanup,
  isCleanupFailureError,
  retryCleanupFromError,
  trackRetryableCleanupOwnership,
} from '../utils/retryable-cleanup';
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
 * import { scan } from 'rolldown/experimental';
 *
 * await scan(...);
 * // Now all resources have been cleaned up.
 * ```
 */
export const scan = async (
  rawInputOptions: InputOptions,
  rawOutputOptions: OutputOptions = {},
): Promise<void> => {
  assertParallelPluginOptionsSupported(rawInputOptions.plugins, rawOutputOptions.plugins);
  validateOption('input', rawInputOptions);
  validateOption('output', rawOutputOptions);

  const inputOptions = await PluginDriver.callOptionsHook(rawInputOptions);
  let ret: Awaited<ReturnType<typeof createBundlerOptions>>;
  try {
    ret = await createBundlerOptions(inputOptions, rawOutputOptions, false);
  } catch (error) {
    if (!isCleanupFailureError(error)) throw error;
    return retryCleanupFromError(
      error,
      'Scan option setup and parallel-plugin worker retry cleanup both failed',
    );
  }

  let stopWorkers = ret.stopWorkers;
  let runtimeLease: RuntimeLease | undefined;
  let bundler: BindingBundler | undefined;
  let nativeClosePromise: Promise<void> | undefined;
  let cleanupAttempt: Promise<void> | undefined;
  const hasRetryableCleanup = () => stopWorkers !== undefined || runtimeLease !== undefined;
  const cleanup = (): Promise<void> =>
    (cleanupAttempt ??= (async () => {
      const errors: unknown[] = [];
      if (bundler) {
        nativeClosePromise ??= (async () => bundler!.close())();
        try {
          await nativeClosePromise;
        } catch (error) {
          errors.push(error);
        }
      }
      const ownedStopWorkers = stopWorkers;
      try {
        await ownedStopWorkers?.();
        if (stopWorkers === ownedStopWorkers) {
          stopWorkers = undefined;
        }
      } catch (error) {
        errors.push(error);
      }
      try {
        runtimeLease?.release();
        runtimeLease = undefined;
      } catch (error) {
        errors.push(error);
      }
      if (errors.length > 0) {
        const cleanupError =
          errors.length === 1
            ? errors[0]
            : new AggregateError(
                errors,
                'Scan native close, parallel-plugin worker shutdown, or runtime release failed',
              );
        if (hasRetryableCleanup()) {
          const retryableError =
            cleanupError instanceof Error
              ? cleanupError
              : new AggregateError([cleanupError], 'Scan cleanup failed with a non-Error value');
          attachRetryableCleanup(retryableError, cleanup);
          throw retryableError;
        }
        throw cleanupError;
      }
    })().finally(() => {
      cleanupAttempt = undefined;
    }));
  trackRetryableCleanupOwnership(cleanup, hasRetryableCleanup);

  const throwAfterCleanupWithRetry = async (
    error: unknown,
    message: string,
    retryMessage: string,
  ): Promise<never> => {
    try {
      await cleanup();
    } catch (cleanupError) {
      const setupError = createCleanupFailureError(
        error,
        cleanupError,
        getRetryableCleanup(cleanupError),
        message,
      );
      return retryCleanupFromError(setupError, retryMessage);
    }
    throw error;
  };

  try {
    runtimeLease = await acquireRuntimeLease();
    bundler = new BindingBundler();
  } catch (error) {
    return throwAfterCleanupWithRetry(
      error,
      'Scan setup and cleanup failed',
      'Scan setup and retry cleanup both failed',
    );
  }

  try {
    const result = await bundler!.scan(ret.bundlerOptions);
    unwrapBindingResult(result);
  } catch (error) {
    return throwAfterCleanupWithRetry(
      error,
      'Scan and cleanup both failed',
      'Scan and retry cleanup both failed',
    );
  }

  try {
    await cleanup();
  } catch (error) {
    return retryCleanupFromError(error, 'Scan cleanup retry failed');
  }
};
