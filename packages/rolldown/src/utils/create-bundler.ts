import {
  type BindingBundler,
  type BindingBundlerImpl,
  shutdownAsyncRuntime,
  startAsyncRuntime,
} from '../binding';
import type { InputOptions } from '../options/input-options';
import type { OutputOptions } from '../options/output-options';
import { createBundlerOptions } from './create-bundler-option';

let asyncRuntimeShutdown = false;

export async function createBundlerImpl(
  bundler: BindingBundler,
  inputOptions: InputOptions,
  outputOptions: OutputOptions,
  isClose?: boolean,
): Promise<BundlerImplWithStopWorker> {
  const option = await createBundlerOptions(
    inputOptions,
    outputOptions,
    false,
    isClose,
  );

  if (asyncRuntimeShutdown) {
    startAsyncRuntime();
  }

  try {
    return {
      impl: bundler.createImpl(option.bundlerOptions),
      stopWorkers: option.stopWorkers,
      shutdown: () => {
        shutdownAsyncRuntime();
        asyncRuntimeShutdown = true;
      },
    };
  } catch (e) {
    await option.stopWorkers?.();
    throw e;
  }
}

export interface BundlerImplWithStopWorker {
  impl: BindingBundlerImpl;
  stopWorkers?: () => Promise<void>;
  shutdown: () => void;
}
