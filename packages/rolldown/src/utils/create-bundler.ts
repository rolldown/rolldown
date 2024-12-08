import { Bundler } from '../binding'
import type { InputOptions } from '../options/input-options'
import type { OutputOptions } from '../options/output-options'
import { createBundlerOptions } from './create-bundler-option'

export async function createBundler(
  inputOptions: InputOptions,
  outputOptions: OutputOptions,
): Promise<BundlerWithStopWorker> {
  const option = await createBundlerOptions(inputOptions, outputOptions)

  try {
    return {
      bundler: new Bundler(option.bundlerOptions),
      stopWorkers: option.stopWorkers,
    }
  } catch (e) {
    await option.stopWorkers?.()
    throw e
  }
}

export interface BundlerWithStopWorker {
  bundler: Bundler
  stopWorkers?: () => Promise<void>
}
