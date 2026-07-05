import { BindingBundler } from '../../binding.cjs';
import type { InputOptions } from '../../options/input-options';
import type { OutputOptions } from '../../options/output-options';
import {
  acquireRuntimeLease,
  CloseCoordinator,
  type CloseAttemptResult,
  type RuntimeLease,
} from '../../runtime-lifecycle';
import type { HasProperty, TypeAssert } from '../../types/assert';
import type { RolldownOutput } from '../../types/rolldown-output';
import { RolldownOutputImpl } from '../../types/rolldown-output-impl';
import { createBundlerOptions } from '../../utils/create-bundler-option';
import { unwrapBindingResult } from '../../utils/error';
import { validateOption } from '../../utils/validator';
// oxlint-disable-next-line no-unused-vars -- this is used in JSDoc links
import type { rolldown } from './index';
// oxlint-disable-next-line no-unused-vars -- this is used in JSDoc links
import type { BundleError } from '../../utils/error';

// @ts-expect-error TS2540: the polyfill of `asyncDispose`.
Symbol.asyncDispose ??= Symbol('Symbol.asyncDispose');

/**
 * The bundle object returned by {@linkcode rolldown} function.
 *
 * @category Programmatic APIs
 */
export class RolldownBuild {
  #inputOptions: InputOptions;
  #bundler: BindingBundler;
  #runtimeLease: RuntimeLease;
  #stopWorkers?: () => Promise<void>;
  #nativeClosePromise: Promise<void> | undefined;
  #closeCoordinator = new CloseCoordinator(
    'Bundle native close, parallel-plugin worker shutdown, or runtime release failed',
  );

  /** @hidden should not be used directly */
  constructor(inputOptions: InputOptions) {
    this.#inputOptions = inputOptions;
    this.#runtimeLease = acquireRuntimeLease();
    try {
      this.#bundler = new BindingBundler();
    } catch (error) {
      try {
        this.#runtimeLease.release();
      } catch (cleanupError) {
        throw new AggregateError(
          [error, cleanupError],
          'Bundle construction and runtime release both failed',
        );
      }
      throw error;
    }
  }

  /**
   * Whether the bundle has been closed.
   *
   * If the bundle is closed, calling other methods will throw an error.
   */
  get closed(): boolean {
    return this.#bundler.closed;
  }

  /**
   * Generate bundles in-memory.
   *
   * If you directly want to write bundles to disk, use the {@linkcode write} method instead.
   *
   * @param outputOptions The output options.
   * @returns The generated bundle.
   * @throws {@linkcode BundleError} When an error occurs during the build.
   */
  async generate(outputOptions: OutputOptions = {}): Promise<RolldownOutput> {
    return this.#build(false, outputOptions);
  }

  /**
   * Generate and write bundles to disk.
   *
   * If you want to generate bundles in-memory, use the {@linkcode generate} method instead.
   *
   * @param outputOptions The output options.
   * @returns The generated bundle.
   * @throws {@linkcode BundleError} When an error occurs during the build.
   */
  async write(outputOptions: OutputOptions = {}): Promise<RolldownOutput> {
    return this.#build(true, outputOptions);
  }

  /**
   * Close the bundle and free resources.
   *
   * This method should be called even if the {@linkcode generate} method
   * or the {@linkcode write} method threw an error. It should be called
   * even if neither of the methods are called.
   *
   * This method is called automatically when using `using` syntax.
   *
   * @example
   * ```js
   * import { rolldown } from 'rolldown';
   *
   * {
   *   using bundle = await rolldown({ input: 'src/main.js' });
   *   const output = await bundle.generate({ format: 'esm' });
   *   console.log(output);
   *   // bundle.close() is called automatically here
   * }
   * ```
   */
  close(): Promise<void> {
    return this.#closeCoordinator.close(() => this.#close());
  }

  async #close(): Promise<CloseAttemptResult> {
    const errors: unknown[] = [];
    let retryable = false;
    this.#nativeClosePromise ??= (async () => this.#bundler.close())();
    try {
      await this.#nativeClosePromise;
    } catch (error) {
      errors.push(error);
    }

    const stopWorkers = this.#stopWorkers;
    try {
      await stopWorkers?.();
      if (this.#stopWorkers === stopWorkers) {
        this.#stopWorkers = undefined;
      }
    } catch (error) {
      errors.push(error);
      retryable = true;
    }

    try {
      this.#runtimeLease.release();
    } catch (error) {
      errors.push(error);
      retryable = true;
    }

    return { errors, retryable };
  }

  /** @hidden documented in close method */
  async [Symbol.asyncDispose](): Promise<void> {
    await this.close();
  }

  // TODO(shulaoda)
  // The `watchFiles` method returns a promise, but Rollup does not.
  // Converting it to a synchronous API might cause a deadlock if the user calls `write` and `watchFiles` simultaneously.
  /**
   * @experimental
   * @hidden not ready for public usage yet
   */
  get watchFiles(): Promise<string[]> {
    return Promise.resolve(this.#bundler.getWatchFiles());
  }

  async #build(isWrite: boolean, outputOptions: OutputOptions): Promise<RolldownOutput> {
    validateOption('output', outputOptions);
    const previousStopWorkers = this.#stopWorkers;
    if (previousStopWorkers) {
      await previousStopWorkers();
      if (this.#stopWorkers === previousStopWorkers) {
        this.#stopWorkers = undefined;
      }
    }
    const option = await createBundlerOptions(this.#inputOptions, outputOptions, false);

    try {
      this.#stopWorkers = option.stopWorkers;
      let output: Awaited<ReturnType<BindingBundler['generate']>>;
      if (isWrite) {
        output = await this.#bundler.write(option.bundlerOptions);
      } else {
        output = await this.#bundler.generate(option.bundlerOptions);
      }
      return new RolldownOutputImpl(unwrapBindingResult(output));
    } catch (e) {
      try {
        await option.stopWorkers?.();
        if (this.#stopWorkers === option.stopWorkers) {
          this.#stopWorkers = undefined;
        }
      } catch (cleanupError) {
        throw new AggregateError(
          [e, cleanupError],
          'Bundle build and parallel-plugin worker cleanup both failed',
        );
      }
      throw e;
    }
  }
}

function _assert() {
  type _ = TypeAssert<HasProperty<RolldownBuild, 'generate' | 'write'>>;
}
