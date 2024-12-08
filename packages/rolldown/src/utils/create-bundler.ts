import { Bundler } from '../binding'
import type { InputOptions } from '../options/input-options'
import type { OutputOptions } from '../options/output-options'
import { createBundlerOption } from './create-bundler-option'

export async function createBundler(
  inputOptions: InputOptions,
  outputOptions: OutputOptions,
): Promise<BundlerWithStopWorker> {
  const option = await createBundlerOption(inputOptions, outputOptions)

  try {
    return {
      bundler: new Bundler(option.bundlerOption),
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
