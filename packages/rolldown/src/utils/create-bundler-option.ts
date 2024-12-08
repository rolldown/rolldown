import { BindingBundlerOptions } from '../binding'
import { PluginDriver } from '../plugin/plugin-driver'
import { TreeshakingOptionsSchema } from '../treeshake'
import { bindingifyInputOptions } from './bindingify-input-options'
import { bindingifyOutputOptions } from './bindingify-output-options'
import { composeJsPlugins } from './compose-js-plugins'
import {
  ANONYMOUS_OUTPUT_PLUGIN_PREFIX,
  ANONYMOUS_PLUGIN_PREFIX,
  checkOutputPluginOption,
  normalizePluginOption,
  normalizePlugins,
} from './normalize-plugin-option'
import { initializeParallelPlugins } from './initialize-parallel-plugins'
import type { InputOptions } from '../options/input-options'
import type { OutputOptions } from '../options/output-options'
import { LOG_LEVEL_INFO } from '../log/logging'
import { getLogger, getOnLog } from '../log/logger'
import { getObjectPlugins } from '../plugin/plugin-driver'

export async function createBundlerOptions(
  inputOptions: InputOptions,
  outputOptions: OutputOptions,
): Promise<BundlerOptionWithStopWorker> {
  const pluginDriver = new PluginDriver()
  inputOptions = await pluginDriver.callOptionsHook(inputOptions)
  if (inputOptions.treeshake !== undefined) {
    TreeshakingOptionsSchema.parse(inputOptions.treeshake)
  }

  const inputPlugins = await normalizePluginOption(inputOptions.plugins)

  const outputPlugins = await normalizePluginOption(outputOptions.plugins)

  // The `outputOptions` hook is called with the input plugins and the output plugins
  outputOptions = pluginDriver.callOutputOptionsHook(
    [...inputPlugins, ...outputPlugins],
    outputOptions,
  )

  const logLevel = inputOptions.logLevel || LOG_LEVEL_INFO
  const onLog = getLogger(
    getObjectPlugins(inputPlugins),
    getOnLog(inputOptions, logLevel),
    logLevel,
  )

  let plugins = [
    ...normalizePlugins(inputPlugins, ANONYMOUS_PLUGIN_PREFIX),
    ...checkOutputPluginOption(
      normalizePlugins(
        await normalizePluginOption(outputOptions.plugins),
        ANONYMOUS_OUTPUT_PLUGIN_PREFIX,
      ),
      onLog,
    ),
  ]

  if (inputOptions.experimental?.enableComposingJsPlugins ?? false) {
    plugins = composeJsPlugins(plugins)
  }

  const parallelPluginInitResult = await initializeParallelPlugins(plugins)

  try {
    // Convert `InputOptions` to `BindingInputOptions`
    const bindingInputOptions = bindingifyInputOptions(
      plugins,
      inputOptions,
      outputOptions,
      onLog,
      logLevel,
    )

    // Convert `OutputOptions` to `BindingInputOptions`
    const bindingOutputOptions = bindingifyOutputOptions(outputOptions)

    return {
      bundlerOptions: {
        inputOptions: bindingInputOptions,
        outputOptions: bindingOutputOptions,
        parallelPluginsRegistry: parallelPluginInitResult?.registry,
      },
      stopWorkers: parallelPluginInitResult?.stopWorkers,
    }
  } catch (e) {
    await parallelPluginInitResult?.stopWorkers()
    throw e
  }
}

export interface BundlerOptionWithStopWorker {
  bundlerOptions: BindingBundlerOptions
  stopWorkers?: () => Promise<void>
}
