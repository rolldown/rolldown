import { bindingifyInputOptions } from '../options/bindingify-input-options'
import { Bundler } from '../binding'
import { initializeParallelPlugins } from './initialize-parallel-plugins'
import { normalizeInputOptions } from './normalize-input-options'
import { bindingifyOutputOptions } from '../options/bindingify-output-options'
import { PluginDriver } from '../plugin/plugin-driver'
import { TreeshakingOptionsSchema } from '../treeshake'
import { normalizePluginOption } from './normalize-plugin-option'
import { composeJsPlugins } from './compose-js-plugins'
import type { InputOptions } from '../types/input-options'
import type { OutputOptions } from '../types/output-options'

export async function createBundler(
  inputOptions: InputOptions,
  outputOptions: OutputOptions,
): Promise<BundlerWithStopWorker> {
  const pluginDriver = new PluginDriver()
  inputOptions = await pluginDriver.callOptionsHook(inputOptions)
  if (inputOptions.treeshake !== undefined) {
    TreeshakingOptionsSchema.parse(inputOptions.treeshake)
  }

  // Convert `RolldownPluginRec` to `RolldownPlugin`
  let plugins = await normalizePluginOption(inputOptions.plugins)
  if (inputOptions.experimental?.enableComposingJsPlugins ?? false) {
    plugins = composeJsPlugins(plugins)
  }

  // Convert `InputOptions` to `NormalizedInputOptions`.
  const normalizedInputOptions = await normalizeInputOptions(
    inputOptions,
    plugins,
  )

  const parallelPluginInitResult = await initializeParallelPlugins(plugins)

  try {
    outputOptions = pluginDriver.callOutputOptionsHook(plugins, outputOptions)

    // Convert `NormalizedInputOptions` to `BindingInputOptions`
    const bindingInputOptions = bindingifyInputOptions(
      normalizedInputOptions,
      outputOptions,
      plugins,
    )

    // Convert `NormalizedOutputOptions` to `BindingInputOptions`
    const bindingOutputOptions = bindingifyOutputOptions(outputOptions)

    return {
      bundler: new Bundler(
        bindingInputOptions,
        bindingOutputOptions,
        parallelPluginInitResult?.registry,
      ),
      stopWorkers: parallelPluginInitResult?.stopWorkers,
    }
  } catch (e) {
    await parallelPluginInitResult?.stopWorkers()
    throw e
  }
}

export interface BundlerWithStopWorker {
  bundler: Bundler
  stopWorkers?: () => Promise<void>
}
