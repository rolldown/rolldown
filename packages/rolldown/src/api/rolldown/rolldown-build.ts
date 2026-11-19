import { BindingBundler, shutdownAsyncRuntime, startAsyncRuntime } from '../../binding.cjs';
import type { InputOptions } from '../../options/input-options';
import type { OutputOptions } from '../../options/output-options';
import type { HasProperty, TypeAssert } from '../../types/assert';
import type { RolldownOutput } from '../../types/rolldown-output';
import { RolldownOutputImpl } from '../../types/rolldown-output-impl';
import { createBundlerOptions } from '../../utils/create-bundler-option';
import { unwrapBindingResult } from '../../utils/error';
import { noop } from '../../utils/misc';
import { trackOutputExternalMemory } from '../../utils/transform-to-rollup-output';
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
  #closing?: Promise<void>;

  /** @hidden should not be used directly */
  constructor(inputOptions: InputOptions) {
    this.#inputOptions = inputOptions;
    this.#bundler = new BindingBundler();
    startAsyncRuntime();
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
  async close(): Promise<void> {
    // Every cleanup step (`BindingBundler.close`, release the async runtime) runs exactly once,
    // even when the native close fails, and a repeated `close` never re-runs any of them. The first
    // caller gets any error; a repeat only waits.
    // See internal-docs/rust-classic-bundler/implementation.md ("Close Mechanism").
    if (this.#closing) {
      await this.#closing.catch(noop);
      return;
    }
    this.#closing = this.#close();
    await this.#closing;
  }

  async #close(): Promise<void> {
    try {
      // `BindingBundler.close` spawns onto the runtime, so release only after it settles.
      await this.#bundler.close();
    } finally {
      shutdownAsyncRuntime();
    }
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
    const bundlerOptions = await createBundlerOptions(
      this.#inputOptions,
      outputOptions,
      /* watchMode */ false,
      /* measureTimings */ true,
    );
    const output = isWrite
      ? await this.#bundler.write(bundlerOptions)
      : await this.#bundler.generate(bundlerOptions);
    const bindingOutputs = unwrapBindingResult(output);
    trackOutputExternalMemory(bindingOutputs);
    return new RolldownOutputImpl(bindingOutputs);
  }
}

function _assert() {
  type _ = TypeAssert<HasProperty<RolldownBuild, 'generate' | 'write'>>;
}
