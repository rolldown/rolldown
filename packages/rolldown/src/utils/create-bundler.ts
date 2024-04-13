import { Bundler } from '../binding'
import {
  normalizeInputOptions,
  type InputOptions,
} from '../options/input-options'
import { createInputOptionsAdapter } from '../options/input-options-adapter'
import {
  OutputOptions,
  normalizeOutputOptions,
} from '../options/output-options'
import { initializeParallelPlugins } from './initialize-parallel-plugins'

export async function createBundler(
  inputOptions: InputOptions,
  outputOptions: OutputOptions,
): Promise<{ bundler: Bundler; stopWorkers?: () => Promise<void> }> {
  // Convert `InputOptions` to `NormalizedInputOptions`.
  const normalizedInputOptions = await normalizeInputOptions(inputOptions)

  const parallelPluginInitResult = await initializeParallelPlugins(
    normalizedInputOptions.plugins,
  )

  const normalizedOutputOptions = normalizeOutputOptions(outputOptions)
  // Convert `NormalizedInputOptions` to `BindingInputOptions`
  const bindingInputOptions = createInputOptionsAdapter(
    normalizedInputOptions,
    inputOptions,
    normalizedOutputOptions,
  )

  return {
    bundler: new Bundler(
      bindingInputOptions,
      normalizedOutputOptions,
      parallelPluginInitResult?.registry,
    ),
    stopWorkers: parallelPluginInitResult?.stopWorkers,
  }
}
