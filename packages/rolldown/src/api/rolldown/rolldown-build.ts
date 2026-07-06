import { BindingBundler } from '../../binding.cjs';
import type { InputOptions } from '../../options/input-options';
import type { OutputOptions } from '../../options/output-options';
import {
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

interface BuildOperation {
  settled: boolean;
  stopPromise?: Promise<void>;
  stopWorkers?: () => Promise<void>;
}

interface BuildCleanupOwnership {
  runtimeLeaseReleased: boolean;
  workerOwners: Set<BuildOperation>;
}

const buildCleanupOwnership = new WeakMap<RolldownBuild, BuildCleanupOwnership>();

/** @internal */
export function hasRetryableBuildCleanup(build: RolldownBuild): boolean {
  const ownership = buildCleanupOwnership.get(build);
  return (
    ownership !== undefined && (!ownership.runtimeLeaseReleased || ownership.workerOwners.size > 0)
  );
}

/**
 * The bundle object returned by {@linkcode rolldown} function.
 *
 * @category Programmatic APIs
 */
export class RolldownBuild {
  #inputOptions: InputOptions;
  #bundler: BindingBundler;
  #runtimeLease: RuntimeLease;
  #activeBuilds = new Set<Promise<RolldownOutput>>();
  #workerOwners = new Set<BuildOperation>();
  #latestBuildOperation: BuildOperation | undefined;
  #nativeEntryQueue: Promise<void> = Promise.resolve();
  #nativeClosePromise: Promise<void> | undefined;
  #closeRequested = false;
  #closeCoordinator = new CloseCoordinator(
    'Bundle native close, parallel-plugin worker shutdown, or runtime release failed',
  );

  /** @hidden should not be used directly */
  constructor(inputOptions: InputOptions, runtimeLease: RuntimeLease) {
    this.#inputOptions = inputOptions;
    this.#runtimeLease = runtimeLease;
    buildCleanupOwnership.set(this, {
      runtimeLeaseReleased: false,
      workerOwners: this.#workerOwners,
    });
    try {
      this.#bundler = new BindingBundler();
    } catch (error) {
      try {
        this.#runtimeLease.release();
      } catch (cleanupError) {
        throw new AggregateError(
          [error, cleanupError],
          'Bundle construction and runtime release both failed',
          { cause: error },
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
    return this.#closeRequested || this.#bundler.closed;
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
    return this.#startBuild(false, outputOptions);
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
    return this.#startBuild(true, outputOptions);
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
    this.#closeRequested = true;
    return this.#closeCoordinator.close(() => this.#close());
  }

  async #close(): Promise<CloseAttemptResult> {
    const errors: unknown[] = [];
    let retryable = false;
    await Promise.allSettled(this.#activeBuilds);

    const latestWorkerOwner = this.#latestBuildOperation;
    for (const owner of this.#workerOwners) {
      if (owner === latestWorkerOwner) continue;
      try {
        await this.#stopWorkerOwner(owner);
      } catch (error) {
        errors.push(error);
        retryable = true;
      }
    }

    this.#nativeClosePromise ??= (async () => this.#bundler.close())();
    try {
      await this.#nativeClosePromise;
    } catch (error) {
      errors.push(error);
    }

    if (latestWorkerOwner && this.#workerOwners.has(latestWorkerOwner)) {
      try {
        await this.#stopWorkerOwner(latestWorkerOwner);
      } catch (error) {
        errors.push(error);
        retryable = true;
      }
    }

    const cleanupOwnership = buildCleanupOwnership.get(this)!;
    if (!cleanupOwnership.runtimeLeaseReleased) {
      try {
        this.#runtimeLease.release();
        cleanupOwnership.runtimeLeaseReleased = true;
      } catch (error) {
        errors.push(error);
        retryable = true;
      }
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

  #startBuild(isWrite: boolean, outputOptions: OutputOptions): Promise<RolldownOutput> {
    if (this.#closeRequested) {
      return Promise.reject(
        new Error(
          '[ALREADY_CLOSED] Bundle is already closed, no more calls to "generate" or "write" are allowed.\n',
        ),
      );
    }

    const result = this.#build(isWrite, outputOptions);
    this.#activeBuilds.add(result);
    void result.then(
      () => this.#activeBuilds.delete(result),
      () => this.#activeBuilds.delete(result),
    );
    return result;
  }

  async #build(isWrite: boolean, outputOptions: OutputOptions): Promise<RolldownOutput> {
    validateOption('output', outputOptions);
    const option = await createBundlerOptions(this.#inputOptions, outputOptions, false);
    const operation: BuildOperation = {
      settled: false,
      stopWorkers: option.stopWorkers,
    };
    if (operation.stopWorkers) {
      this.#workerOwners.add(operation);
    }

    let result: RolldownOutput;
    let supersededCleanupErrors: unknown[] = [];
    try {
      const nativeBuild = await this.#enterNativeBuild(operation, isWrite, option.bundlerOptions);
      supersededCleanupErrors = nativeBuild.supersededCleanupErrors;
      result = new RolldownOutputImpl(unwrapBindingResult(await nativeBuild.nativePromise));
    } catch (error) {
      const errors: unknown[] = [error];
      operation.settled = true;
      if (this.#latestBuildOperation !== operation) {
        try {
          await this.#stopWorkerOwner(operation);
        } catch (caughtCleanupError) {
          errors.push(caughtCleanupError);
        }
      }
      // The latest native BundleHandle still needs its parallel closeBundle
      // hooks after a rejected build. See internal-docs/async-runtime/implementation.md.
      errors.push(...supersededCleanupErrors);
      if (errors.length > 1) {
        throw new AggregateError(
          errors,
          'Bundle build and parallel-plugin worker cleanup both failed',
          { cause: error },
        );
      }
      throw error;
    }

    operation.settled = true;
    const cleanupErrors = [...supersededCleanupErrors];
    if (this.#latestBuildOperation !== operation) {
      try {
        await this.#stopWorkerOwner(operation);
      } catch (error) {
        cleanupErrors.push(error);
      }
    }
    if (cleanupErrors.length === 1) {
      throw cleanupErrors[0];
    }
    if (cleanupErrors.length > 1) {
      throw new AggregateError(
        cleanupErrors,
        'Multiple parallel-plugin worker cleanup attempts failed',
        { cause: cleanupErrors[0] },
      );
    }
    return result;
  }

  #enterNativeBuild(
    operation: BuildOperation,
    isWrite: boolean,
    bundlerOptions: Parameters<BindingBundler['generate']>[0],
  ): Promise<{
    nativePromise: ReturnType<BindingBundler['generate']>;
    supersededCleanupErrors: unknown[];
  }> {
    const entry = this.#nativeEntryQueue.then(async () => {
      const previous = this.#latestBuildOperation;
      const nativePromise = isWrite
        ? this.#bundler.write(bundlerOptions)
        : this.#bundler.generate(bundlerOptions);
      this.#latestBuildOperation = operation;

      const supersededCleanupErrors: unknown[] = [];
      if (previous?.settled) {
        try {
          // Native entry synchronously installs this operation's bundle
          // handle. Retire the previous workers only after that replacement
          // is visible, so a failed retirement cannot leave native closeBundle
          // targeting a partially terminated worker pool.
          await this.#stopWorkerOwner(previous);
        } catch (error) {
          supersededCleanupErrors.push(error);
        }
      }
      return { nativePromise, supersededCleanupErrors };
    });
    this.#nativeEntryQueue = entry.then(
      () => {},
      () => {},
    );
    return entry;
  }

  async #stopWorkerOwner(owner: BuildOperation): Promise<void> {
    const stopWorkers = owner.stopWorkers;
    if (!stopWorkers) return;

    owner.stopPromise ??= stopWorkers();
    try {
      await owner.stopPromise;
    } catch (error) {
      owner.stopPromise = undefined;
      throw error;
    }
    owner.stopPromise = undefined;
    owner.stopWorkers = undefined;
    this.#workerOwners.delete(owner);
  }
}

function _assert() {
  type _ = TypeAssert<HasProperty<RolldownBuild, 'generate' | 'write'>>;
}
