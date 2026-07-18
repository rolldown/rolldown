import type { InputOptions } from '../options/input-options';
import type { OutputOptions } from '../options/output-options';
import { assertParallelPluginOptionsSupported } from '../plugin/parallel-plugin';
import type { RolldownOutput } from '../types/rolldown-output';
import { getCloseTerminalErrors, throwCloseErrors } from '../runtime-lifecycle';
import {
  attachRetryableCleanup,
  createCleanupFailureError,
  excludeDeliveredErrors,
  getRetryableCleanup,
  retryCleanupFromError,
  trackRetryableCleanupOwnership,
  waitForRetryableCleanupTurn,
} from '../utils/retryable-cleanup';
import { rolldown } from './rolldown';
import {
  hasRetryableBuildCleanup,
  retryRolldownBuildCleanup,
  type RolldownBuild,
} from './rolldown/rolldown-build';

/**
 * The options for {@linkcode build} function.
 *
 * @experimental
 * @category Programmatic APIs
 */
export type BuildOptions = InputOptions & {
  /**
   * Write the output to the file system
   *
   * @default true
   */
  write?: boolean;
  output?: OutputOptions;
};

/**
 * Build a single output.
 *
 * @param options The build options.
 * @returns A Promise that resolves to the build output.
 */
async function build(options: BuildOptions): Promise<RolldownOutput>;
/**
 * Build multiple outputs __sequentially__.
 *
 * @param options The build options.
 * @returns A Promise that resolves to the build outputs for each option.
 */
async function build(options: BuildOptions[]): Promise<RolldownOutput[]>;
/**
 * The API similar to esbuild's `build` function.
 *
 * @example
 * ```js
 * import { build } from 'rolldown';
 *
 * const result = await build({
 *   input: 'src/main.js',
 *   output: {
 *     file: 'bundle.js',
 *   },
 * });
 * console.log(result);
 * ```
 *
 * @experimental
 * @category Programmatic APIs
 */
async function build(
  options: BuildOptions | BuildOptions[],
): Promise<RolldownOutput | RolldownOutput[]> {
  for (const option of Array.isArray(options) ? options : [options]) {
    assertParallelPluginOptionsSupported(option.plugins, option.output?.plugins);
  }
  if (Array.isArray(options)) {
    const outputs: RolldownOutput[] = [];
    for (const option of options) {
      outputs.push(await build(option));
    }
    return outputs;
  } else {
    const { output, write = true, ...inputOptions } = options;
    const build = await rolldown(inputOptions);
    let result!: RolldownOutput;
    let buildError: unknown;
    let buildFailed = false;
    try {
      if (write) {
        result = await build.write(output);
      } else {
        result = await build.generate(output);
      }
    } catch (error) {
      buildFailed = true;
      buildError = error;
    }

    let closeError: unknown;
    let closeFailed = false;
    try {
      await closeBuild(build);
    } catch (error) {
      closeFailed = true;
      closeError = error;
    }

    if (buildFailed && closeFailed) {
      throw createCleanupFailureError(
        buildError,
        closeError,
        getRetryableCleanup(closeError),
        'Build and cleanup both failed',
      );
    }
    if (buildFailed) {
      throw buildError;
    }
    if (closeFailed) {
      throw closeError;
    }
    return result;
  }
}

async function closeBuild(build: RolldownBuild): Promise<void> {
  const deliveredTerminalErrors: unknown[] = [];
  const cleanup = async () => {
    let terminalErrors: unknown[];
    try {
      terminalErrors = await retryRolldownBuildCleanup(build);
    } catch (error) {
      const newlyDeliveredTerminalErrors = excludeDeliveredErrors(
        getCloseTerminalErrors(error),
        deliveredTerminalErrors,
      );
      deliveredTerminalErrors.push(...newlyDeliveredTerminalErrors);
      if (newlyDeliveredTerminalErrors.length === 0) throw error;
      throw new AggregateError(
        [...newlyDeliveredTerminalErrors, error],
        'Build cleanup retry delivered terminal diagnostics and cleanup failures',
        { cause: newlyDeliveredTerminalErrors[0] },
      );
    }
    const newlyDeliveredTerminalErrors = excludeDeliveredErrors(
      terminalErrors,
      deliveredTerminalErrors,
    );
    deliveredTerminalErrors.push(...newlyDeliveredTerminalErrors);
    throwCloseErrors(newlyDeliveredTerminalErrors, 'Build close failed');
  };
  trackRetryableCleanupOwnership(cleanup, () => hasRetryableBuildCleanup(build), {
    recoverAbandoned: false,
  });

  try {
    await build.close();
  } catch (error) {
    if (!hasRetryableBuildCleanup(build)) throw error;
    const initialTerminalErrors = [...getCloseTerminalErrors(error)];
    deliveredTerminalErrors.push(...initialTerminalErrors);
    const retryableError =
      error instanceof Error
        ? error
        : new AggregateError([error], 'Build cleanup failed with a non-Error value');
    attachRetryableCleanup(retryableError, cleanup);

    let retryError: unknown;
    try {
      await retryCleanupFromError(retryableError, 'Build cleanup and retry both failed');
    } catch (caughtError) {
      retryError = caughtError;
    }
    if (!getRetryableCleanup(retryError) && !hasRetryableBuildCleanup(build)) {
      throwCloseErrors(deliveredTerminalErrors, 'Build close failed');
      return;
    }
    if (!getRetryableCleanup(retryError)) throw retryError;

    await waitForRetryableCleanupTurn();
    let finalRetryError: unknown;
    try {
      await retryCleanupFromError(retryError, 'Build cleanup and final retry both failed');
    } catch (caughtError) {
      finalRetryError = caughtError;
    }
    if (!getRetryableCleanup(finalRetryError) && !hasRetryableBuildCleanup(build)) {
      throwCloseErrors(deliveredTerminalErrors, 'Build close failed');
      return;
    }
    throw finalRetryError;
  }
}

export { build };
