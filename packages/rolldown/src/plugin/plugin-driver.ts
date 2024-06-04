import { getLogHandler, normalizeLog } from '../log/logHandler'
import { LOG_LEVEL_DEBUG, LOG_LEVEL_INFO, LOG_LEVEL_WARN } from '../log/logging'
import { Plugin } from './'
import { error, logPluginError } from '../log/logs'
import { NormalizedInputOptions } from '../options/normalized-input-options'
import { NormalizedOutputOptions } from '../options/normalized-output-options'
import { RollupError } from '@src/rollup'
import { normalizeHook } from '../utils/normalize-hook'

export class PluginDriver {
  public callOptionsHook(inputOptions: NormalizedInputOptions) {
    const logLevel = inputOptions.logLevel
    const plugins = inputOptions.plugins.filter(
      (plugin) => !('_parallel' in plugin),
    ) as Plugin[]
    const logger = inputOptions.onLog

    for (const plugin of plugins) {
      const name = plugin.name || 'unknown'
      const options = plugin.options
      if (options) {
        const [handler, _optionsIgnoredSofar] = normalizeHook(options)
        handler.call(
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
          // TODO Here only support readonly access to the inputOptions at now
          inputOptions,
        )
      }
    }
  }

  public callOutputOptionsHook(
    inputOptions: NormalizedInputOptions,
    outputOptions: NormalizedOutputOptions,
  ) {
    const plugins = inputOptions.plugins.filter(
      (plugin) => !('_parallel' in plugin),
    ) as Plugin[]

    for (const plugin of plugins) {
      const options = plugin.outputOptions
      if (options) {
        const [handler, _optionsIgnoredSofar] = normalizeHook(options)
        handler.call(null, outputOptions)
      }
    }
  }
}
