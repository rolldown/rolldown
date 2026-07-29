import { BindingBundler, type BindingResult } from '../binding.cjs';
import type { InputOptions } from '../options/input-options';
import type { OutputOptions } from '../options/output-options';
import { assertParallelPluginOptionsSupported } from '../plugin/parallel-plugin';
import { PluginDriver } from '../plugin/plugin-driver';
import { acquireRuntimeLease, type RuntimeLease } from '../runtime-lifecycle';
import { createBundlerOptions } from '../utils/create-bundler-option';
import { normalizeBindingResultErrors, unwrapBindingResult } from '../utils/error';
import {
  attachRetryableCleanup,
  createCleanupFailureError,
  excludeDeliveredErrors,
  getRetryableCleanup,
  isCleanupFailureError,
  retryCleanupFromError,
  trackRetryableCleanupOwnership,
  waitForRetryableCleanupTurn,
} from '../utils/retryable-cleanup';
import { validateOption } from '../utils/validator';

export { freeExternalMemory } from '../types/external-memory-handle';

type BindingBundlerWithTerminalClose = BindingBundler & {
  closeTerminal(): Promise<BindingResult<void>>;
};

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
  let bundler: BindingBundlerWithTerminalClose | undefined;
  let nativeClosePromise: Promise<Error[]> | undefined;
  const deliveredTerminalErrors: unknown[] = [];
  let resourceCleanupAttempt: Promise<unknown[]> | undefined;
  const releaseResources = (): Promise<unknown[]> =>
    (resourceCleanupAttempt ??= (async () => {
      const errors: unknown[] = [];
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
      return errors;
    })().finally(() => {
      resourceCleanupAttempt = undefined;
    }));
  const throwCleanupErrors = (
    errors: unknown[],
    cleanup: () => Promise<void>,
    hasOwnership: () => boolean,
  ): never => {
    const cleanupError =
      errors.length === 1
        ? errors[0]
        : new AggregateError(
            errors,
            'Scan native close, parallel-plugin worker shutdown, or runtime release failed',
          );
    if (hasOwnership()) {
      const retryableError =
        cleanupError instanceof Error
          ? cleanupError
          : new AggregateError([cleanupError], 'Scan cleanup failed with a non-Error value');
      attachRetryableCleanup(retryableError, cleanup);
      throw retryableError;
    }
    throw cleanupError;
  };

  let setupCleanupAttempt: Promise<void> | undefined;
  const hasSetupCleanup = () => stopWorkers !== undefined || runtimeLease !== undefined;
  const cleanupSetup = (): Promise<void> =>
    (setupCleanupAttempt ??= (async () => {
      const errors = await releaseResources();
      if (errors.length > 0) {
        throwCleanupErrors(errors, cleanupSetup, hasSetupCleanup);
      }
    })().finally(() => {
      setupCleanupAttempt = undefined;
    }));
  trackRetryableCleanupOwnership(cleanupSetup, hasSetupCleanup);

  let scanCleanupAttempt: Promise<void> | undefined;
  const hasScanCleanup = () =>
    bundler !== undefined || stopWorkers !== undefined || runtimeLease !== undefined;
  const cleanupScan = (): Promise<void> =>
    (scanCleanupAttempt ??= (async () => {
      const errors: unknown[] = [];
      if (bundler) {
        nativeClosePromise ??= (async () =>
          normalizeBindingResultErrors(await bundler!.closeTerminal()))();
        try {
          const terminalErrors = excludeDeliveredErrors(
            await nativeClosePromise,
            deliveredTerminalErrors,
          );
          deliveredTerminalErrors.push(...terminalErrors);
          errors.push(...terminalErrors);
          bundler = undefined;
        } catch (error) {
          nativeClosePromise = undefined;
          throwCleanupErrors([error], cleanupScan, hasScanCleanup);
        }
      }
      errors.push(...(await releaseResources()));
      if (errors.length > 0) {
        throwCleanupErrors(errors, cleanupScan, hasScanCleanup);
      }
    })().finally(() => {
      scanCleanupAttempt = undefined;
    }));
  trackRetryableCleanupOwnership(cleanupScan, hasScanCleanup, {
    recoverAbandoned: false,
  });

  const throwAfterCleanupWithRetry = async (
    error: unknown,
    cleanup: () => Promise<void>,
    message: string,
    retryMessage: string,
    awaitFinalRetry = false,
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
      if (!awaitFinalRetry) {
        return retryCleanupFromError(setupError, retryMessage);
      }
      let retryError: unknown;
      try {
        await retryCleanupFromError(setupError, retryMessage);
      } catch (error) {
        retryError = error;
      }
      if (!getRetryableCleanup(retryError)) throw retryError;
      await waitForRetryableCleanupTurn();
      return retryCleanupFromError(retryError, `${retryMessage} after final retry`);
    }
    throw error;
  };

  try {
    runtimeLease = await acquireRuntimeLease();
    bundler = new BindingBundler() as BindingBundlerWithTerminalClose;
  } catch (error) {
    return throwAfterCleanupWithRetry(
      error,
      cleanupSetup,
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
      cleanupScan,
      'Scan and cleanup both failed',
      'Scan and retry cleanup both failed',
      true,
    );
  }

  try {
    await cleanupScan();
  } catch (error) {
    let retryError: unknown;
    try {
      await retryCleanupFromError(error, 'Scan cleanup retry failed');
    } catch (caughtError) {
      retryError = caughtError;
    }
    if (!getRetryableCleanup(retryError)) throw retryError;
    await waitForRetryableCleanupTurn();
    return retryCleanupFromError(retryError, 'Scan cleanup final retry failed');
  }
};
