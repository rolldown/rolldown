import type { InputOptions } from '../options/input-options';
import type { OutputOptions } from '../options/output-options';
import { assertParallelPluginOptionsSupported } from '../plugin/parallel-plugin';
import type { RolldownOutput } from '../types/rolldown-output';
import {
  createCleanupFailureError,
  runRetryableCleanup,
  trackRetryableCleanupOwnership,
} from '../utils/retryable-cleanup';
import { rolldown } from './rolldown';
import { hasRetryableBuildCleanup, type RolldownBuild } from './rolldown/rolldown-build';

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
      throw new AggregateError([buildError, closeError], 'Build and cleanup both failed', {
        cause: buildError,
      });
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
  const cleanup = () => build.close();
  trackRetryableCleanupOwnership(cleanup, () => hasRetryableBuildCleanup(build));

  try {
    await cleanup();
  } catch (error) {
    if (!hasRetryableBuildCleanup(build)) throw error;
    try {
      await runRetryableCleanup(cleanup);
    } catch (retryError) {
      if (!hasRetryableBuildCleanup(build)) throw retryError;
      throw createCleanupFailureError(
        error,
        retryError,
        cleanup,
        'Build cleanup and retry both failed',
      );
    }
  }
}

export { build };
