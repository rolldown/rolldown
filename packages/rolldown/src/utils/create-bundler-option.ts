import { BindingBundlerOptions } from '../binding'
import { PluginDriver } from '../plugin/plugin-driver'
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
import { getObjectPlugins } from '../plugin/plugin-driver'
import { LogHandler } from '../types/misc'
import { logMinifyWarning } from '../log/logs'
import { getLogger, getOnLog } from '../log/logger'
import { validateTreeShakingOptions } from './validator'
import { LOG_LEVEL_INFO, LOG_LEVEL_WARN } from '../log/logging'
import type { InputOptions } from '../options/input-options'
import type { OutputOptions } from '../options/output-options'

export async function createBundlerOptions(
  inputOptions: InputOptions,
  outputOptions: OutputOptions,
): Promise<BundlerOptionWithStopWorker> {
  const pluginDriver = new PluginDriver()
  inputOptions = await pluginDriver.callOptionsHook(inputOptions)
  if (inputOptions.treeshake !== undefined) {
    validateTreeShakingOptions(inputOptions.treeshake)
  }

  const inputPlugins = await normalizePluginOption(inputOptions.plugins)
  const outputPlugins = await normalizePluginOption(outputOptions.plugins)

  const logLevel = inputOptions.logLevel || LOG_LEVEL_INFO
  const onLog = getLogger(
    getObjectPlugins(inputPlugins),
    getOnLog(inputOptions, logLevel),
    logLevel,
  )

  // The `outputOptions` hook is called with the input plugins and the output plugins
  outputOptions = pluginDriver.callOutputOptionsHook(
    [...inputPlugins, ...outputPlugins],
    outputOptions,
  )

  if (outputOptions.minify === true) {
    onLog(LOG_LEVEL_WARN, logMinifyWarning())
  }

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
      inputOptions,
      onLog,
      stopWorkers: parallelPluginInitResult?.stopWorkers,
    }
  } catch (e) {
    await parallelPluginInitResult?.stopWorkers()
    throw e
  }
}

export interface BundlerOptionWithStopWorker {
  bundlerOptions: BindingBundlerOptions
  inputOptions: InputOptions
  onLog: LogHandler
  stopWorkers?: () => Promise<void>
}
