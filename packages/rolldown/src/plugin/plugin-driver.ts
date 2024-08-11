import { getLogHandler, normalizeLog } from '../log/logHandler'
import { LOG_LEVEL_DEBUG, LOG_LEVEL_INFO, LOG_LEVEL_WARN } from '../log/logging'
import { Plugin, RolldownPluginRec } from './'
import { error, logPluginError } from '../log/logs'
import { NormalizedInputOptions } from '../options/normalized-input-options'
import { RollupError } from '../rollup'
import { normalizeHook } from '../utils/normalize-hook'
import { InputOptions, OutputOptions } from '..'
import { getLogger, getOnLog } from '../log/logger'
import { BuiltinPlugin } from './builtin-plugin'

export class PluginDriver {
  public async callOptionsHook(
    inputOptions: InputOptions,
  ): Promise<InputOptions> {
    const logLevel = inputOptions.logLevel || LOG_LEVEL_INFO
    const plugins = getObjectPlugins(inputOptions.plugins ?? [])
    const logger = getLogger(
      plugins,
      getOnLog(inputOptions, logLevel),
      logLevel,
    )

    for (const plugin of plugins) {
      const name = plugin.name || 'unknown'
      const options = plugin.options
      if (options) {
        const { handler } = normalizeHook(options)
        const result = await handler.call(
          {
            debug: getLogHandler(
              LOG_LEVEL_DEBUG,
              'PLUGIN_LOG',
              logger,
              name,
              logLevel,
            ),
            error: (e: RollupError | string) =>
              error(logPluginError(normalizeLog(e), name, { hook: 'onLog' })),
            info: getLogHandler(
              LOG_LEVEL_INFO,
              'PLUGIN_LOG',
              logger,
              name,
              logLevel,
            ),
            // meta: { rollupVersion, watchMode },
            warn: getLogHandler(
              LOG_LEVEL_WARN,
              'PLUGIN_WARNING',
              logger,
              name,
              logLevel,
            ),
          },
          inputOptions,
        )

        if (result) {
          inputOptions = result
        }
      }
    }

    return inputOptions
  }

  public callOutputOptionsHook(
    inputOptions: NormalizedInputOptions,
    outputOptions: OutputOptions,
  ): OutputOptions {
    const plugins = getObjectPlugins(inputOptions.plugins)

    for (const plugin of plugins) {
      const options = plugin.outputOptions
      if (options) {
        const { handler } = normalizeHook(options)
        const result = handler.call(null, outputOptions)

        if (result) {
          outputOptions = result
        }
      }
    }

    return outputOptions
  }
}

export function getObjectPlugins(plugins: RolldownPluginRec[]): Plugin[] {
  return plugins.filter((plugin) => {
    if ('_parallel' in plugin) {
      return undefined
    }
    if (plugin instanceof BuiltinPlugin) {
      return undefined
    }
    return plugin
  }) as Plugin[]
}
