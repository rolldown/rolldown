import { bindingifyInputOptions } from '@src/options/bindingify-input-options'
import { Bundler } from '../binding'
import type { InputOptions } from '../options/input-options'
import type { OutputOptions } from '../options/output-options'
import { initializeParallelPlugins } from './initialize-parallel-plugins'
import { normalizeInputOptions } from './normalize-input-options'
import { normalizeOutputOptions } from './normalize-output-options'
import { bindingifyOutputOptions } from '@src/options/bindingify-output-options'
import { PluginDriver } from '../plugin/plugin-driver'

export async function createBundler(
  inputOptions: InputOptions,
  outputOptions: OutputOptions,
): Promise<{ bundler: Bundler; stopWorkers?: () => Promise<void> }> {
  const pluginDriver = new PluginDriver()
  // Convert `InputOptions` to `NormalizedInputOptions`.
  const normalizedInputOptions = await normalizeInputOptions(inputOptions)

  pluginDriver.callOptionsHook(normalizedInputOptions)

  const parallelPluginInitResult = await initializeParallelPlugins(
    normalizedInputOptions.plugins,
  )

  try {
    const normalizedOutputOptions = normalizeOutputOptions(outputOptions)
    // Convert `NormalizedInputOptions` to `BindingInputOptions`
    const bindingInputOptions = bindingifyInputOptions(
      normalizedInputOptions,
      normalizedOutputOptions,
    )

    return {
      bundler: new Bundler(
        bindingInputOptions,
        bindingifyOutputOptions(normalizedOutputOptions),
        parallelPluginInitResult?.registry,
      ),
      stopWorkers: parallelPluginInitResult?.stopWorkers,
    }
  } catch (e) {
    await parallelPluginInitResult?.stopWorkers()
    throw e
  }
}
