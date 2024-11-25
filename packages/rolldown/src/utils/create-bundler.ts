import { Bundler } from '../binding'
import { PluginDriver } from '../plugin/plugin-driver'
import { TreeshakingOptionsSchema } from '../treeshake'
import { bindingifyInputOptions } from './bindingify-input-options'
import { bindingifyOutputOptions } from './bindingify-output-options'
import { composeJsPlugins } from './compose-js-plugins'
import { normalizePluginOption } from './normalize-plugin-option'
import { initializeParallelPlugins } from './initialize-parallel-plugins'
import type { InputOptions } from '../options/input-options'
import type { OutputOptions } from '../options/output-options'

export async function createBundler(
  inputOptions: InputOptions,
  outputOptions: OutputOptions,
): Promise<BundlerWithStopWorker> {
  const pluginDriver = new PluginDriver()
  inputOptions = await pluginDriver.callOptionsHook(inputOptions)
  if (inputOptions.treeshake !== undefined) {
    TreeshakingOptionsSchema.parse(inputOptions.treeshake)
  }

  const inputPlugins = await normalizePluginOption(inputOptions.plugins)

  try {
    const outputPlugins = await normalizePluginOption(outputOptions.plugins)

    // The `outputOptions` hook is called with the input plugins and the output plugins
    outputOptions = pluginDriver.callOutputOptionsHook(
      [...inputPlugins, ...outputPlugins],
      outputOptions,
    )

    // TODO give warning if `outputOptions.plugins` using build hooks
    let plugins = [
      ...inputPlugins,
      ...(await normalizePluginOption(outputOptions.plugins)),
    ]

    if (inputOptions.experimental?.enableComposingJsPlugins ?? false) {
      plugins = composeJsPlugins(plugins)
    }

    const parallelPluginInitResult = await initializeParallelPlugins(plugins)

    // Convert `NormalizedInputOptions` to `BindingInputOptions`
    const bindingInputOptions = bindingifyInputOptions(
      plugins,
      inputOptions,
      outputOptions,
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
