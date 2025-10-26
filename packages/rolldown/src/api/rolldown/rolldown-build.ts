import type { BindingBundlerImpl } from '../../binding';
import {
  BindingBundler,
  shutdownAsyncRuntime,
  startAsyncRuntime,
} from '../../binding';
import type { InputOptions } from '../../options/input-options';
import type { OutputOptions } from '../../options/output-options';
import type { HasProperty, TypeAssert } from '../../types/assert';
import {
  type ExternalMemoryHandle,
  freeExternalMemory,
} from '../../types/external-memory-handle';
import type { RolldownOutput } from '../../types/rolldown-output';
import { RolldownOutputImpl } from '../../types/rolldown-output-impl';
import { createBundlerOptions } from '../../utils/create-bundler-option';
import { unwrapBindingResult } from '../../utils/error';
import { validateOption } from '../../utils/validator';

interface BundlerImplWithStopWorker {
  impl: BindingBundlerImpl;
  stopWorkers?: () => Promise<void>;
  shutdown: () => void;
}

// @ts-expect-error TS2540: the polyfill of `asyncDispose`.
Symbol.asyncDispose ??= Symbol('Symbol.asyncDispose');

const IS_WEAKREF_SUPPORTED = typeof WeakRef !== 'undefined';

export class RolldownBuild {
  #inputOptions: InputOptions;
  #bundler: BindingBundler;
  #bundlerImpl?: BundlerImplWithStopWorker;
  #externalMemoryHandles: WeakRef<ExternalMemoryHandle>[] = [];

  static asyncRuntimeShutdown = false;

  constructor(inputOptions: InputOptions) {
    this.#inputOptions = inputOptions;
    this.#bundler = new BindingBundler();
  }

  get closed(): boolean {
    return this.#bundlerImpl?.impl.closed ?? true;
  }

  // Create bundler for each `bundle.write/generate`
  async #getBundlerWithStopWorker(
    outputOptions: OutputOptions,
  ): Promise<BundlerImplWithStopWorker> {
    if (this.#bundlerImpl) {
      await this.#bundlerImpl.stopWorkers?.();
    }

    const option = await createBundlerOptions(
      this.#inputOptions,
      outputOptions,
      false,
    );

    if (RolldownBuild.asyncRuntimeShutdown) {
      startAsyncRuntime();
    }

    try {
      return this.#bundlerImpl = {
        impl: this.#bundler.createImpl(option.bundlerOptions),
        stopWorkers: option.stopWorkers,
        shutdown: () => {
          shutdownAsyncRuntime();
          RolldownBuild.asyncRuntimeShutdown = true;
        },
      };
    } catch (e) {
      await option.stopWorkers?.();
      throw e;
    }
  }

  async scan(): Promise<void> {
    const { impl } = await this.#getBundlerWithStopWorker({});
    const output = await impl.scan();
    unwrapBindingResult(output);
  }

  async generate(outputOptions: OutputOptions = {}): Promise<RolldownOutput> {
    validateOption('output', outputOptions);
    const { impl } = await this.#getBundlerWithStopWorker(outputOptions);
    const outputResult = await impl.generate();
    const output = new RolldownOutputImpl(unwrapBindingResult(outputResult));
    this.#storeToExternalMemoryHandle(output);
    return output;
  }

  async write(outputOptions: OutputOptions = {}): Promise<RolldownOutput> {
    validateOption('output', outputOptions);
    const { impl } = await this.#getBundlerWithStopWorker(outputOptions);
    const outputResult = await impl.write();
    const output = new RolldownOutputImpl(unwrapBindingResult(outputResult));
    this.#storeToExternalMemoryHandle(output);
    return output;
  }

  /**
   * Close the build and free resources eagerly.
   *
   * By default, Rolldown will move data of external memory to Node.js heap before freeing the external memory. This allows users to still access the data after closing the build. However, this may lead to increased memory usage in Node.js heap and potential performance overhead due to data copying.
   *
   * If you don't need to access the data after closing the build, you can set `keepDataAlive` to `false` to free the external memory directly without moving data to Node.js heap.
   *
   * **Note:** Automatic cleanup of external memory handles only works in environments that support `WeakRef` (Node.js 14.6+ and modern browsers). In environments without WeakRef support, you can only rely on the garbage collector or manually free them by calling `freeExternalMemory()` from `rolldown/experimental`.
   *
   * @param keepDataAlive - Whether to keep data alive in Node.js heap after closing the build. Default is `true`.
   */
  async close(keepDataAlive = true): Promise<void> {
    if (this.#bundlerImpl) {
      await this.#bundlerImpl.stopWorkers?.();
      await this.#bundlerImpl.impl.close();
      this.#bundlerImpl.shutdown();
      this.#bundlerImpl = void 0;
    }
    if (this.#externalMemoryHandles.length > 0) {
      for (const ref of this.#externalMemoryHandles) {
        const handle = ref.deref();
        if (handle) {
          freeExternalMemory(handle, keepDataAlive);
        }
      }
      this.#externalMemoryHandles = [];
    }
  }

  async [Symbol.asyncDispose](): Promise<void> {
    await this.close();
  }

  // TODO(shulaoda)
  // The `watchFiles` method returns a promise, but Rollup does not.
  // Converting it to a synchronous API might cause a deadlock if the user calls `write` and `watchFiles` simultaneously.
  get watchFiles(): Promise<string[]> {
    return this.#bundlerImpl?.impl.getWatchFiles() ?? Promise.resolve([]);
  }

  #storeToExternalMemoryHandle(handle: ExternalMemoryHandle) {
    // This is a best-effort attempt to help free external memory. If `WeakRef` is not available, we just skip it.
    if (IS_WEAKREF_SUPPORTED) {
      this.#externalMemoryHandles.push(new WeakRef(handle));
    }
  }
}

function _assert() {
  type _ = TypeAssert<HasProperty<RolldownBuild, 'generate' | 'write'>>;
}
