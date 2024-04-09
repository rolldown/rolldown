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
import { initializeThreadSafePlugins } from './initialize-thread-safe-plugins'

export async function createBundler(
  inputOptions: InputOptions,
  outputOptions: OutputOptions,
): Promise<{ bundler: Bundler; stopWorkers?: () => Promise<void> }> {
  // Convert `InputOptions` to `NormalizedInputOptions`.
  const normalizedInputOptions = await normalizeInputOptions(inputOptions)

  const threadSafePluginInitResult = await initializeThreadSafePlugins(
    normalizedInputOptions.plugins,
  )

  // Convert `NormalizedInputOptions` to `BindingInputOptions`
  const bindingInputOptions = createInputOptionsAdapter(
    normalizedInputOptions,
    inputOptions,
  )
  const bindingOutputOptions = normalizeOutputOptions(outputOptions)

  return {
    bundler: new Bundler(
      bindingInputOptions,
      bindingOutputOptions,
      threadSafePluginInitResult?.registry,
    ),
    stopWorkers: threadSafePluginInitResult?.stopWorkers,
  }
}
