import { Bundler, startAsyncRuntime, shutdownAsyncRuntime } from '../binding'
import type { InputOptions } from '../options/input-options'
import type { OutputOptions } from '../options/output-options'
import { createBundlerOptions } from './create-bundler-option'

let asyncRuntimeShutdown = false

export async function createBundler(
  inputOptions: InputOptions,
  outputOptions: OutputOptions,
  isClose?: boolean,
): Promise<BundlerWithStopWorker> {
  const option = await createBundlerOptions(
    inputOptions,
    outputOptions,
    isClose,
  )

  if (asyncRuntimeShutdown) {
    startAsyncRuntime()
  }

  try {
    return {
      bundler: new Bundler(option.bundlerOptions),
      stopWorkers: option.stopWorkers,
      shutdown: () => {
        shutdownAsyncRuntime()
        asyncRuntimeShutdown = true
      },
    }
  } catch (e) {
    await option.stopWorkers?.()
    throw e
  }
}

export interface BundlerWithStopWorker {
  bundler: Bundler
  stopWorkers?: () => Promise<void>
  shutdown: () => void
}
