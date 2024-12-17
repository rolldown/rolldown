import { getLogHandler, normalizeLog } from '../log/logHandler'
import { LOG_LEVEL_DEBUG, LOG_LEVEL_INFO, LOG_LEVEL_WARN } from '../log/logging'
import { Plugin } from './'
import { error, logPluginError } from '../log/logs'
import { RollupError } from '../rollup'
import { normalizeHook } from '../utils/normalize-hook'
import { InputOptions, OutputOptions, RolldownPlugin, VERSION } from '..'
import { getLogger, getOnLog } from '../log/logger'
import { BuiltinPlugin } from '../builtin-plugin/constructors'
import { normalizePluginOption } from '../utils/normalize-plugin-option'

export class PluginDriver {
  public async callOptionsHook(
    inputOptions: InputOptions,
  ): Promise<InputOptions> {
    const logLevel = inputOptions.logLevel || LOG_LEVEL_INFO
    const plugins = getSortedPlugins(
      'options',
      getObjectPlugins(await normalizePluginOption(inputOptions.plugins)),
    )
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
            meta: {
              rollupVersion: '4.23.0',
              rolldownVersion: VERSION,
              watchMode: false,
            },
            warn: getLogHandler(
              LOG_LEVEL_WARN,
              'PLUGIN_WARNING',
              logger,
              name,
              logLevel,
            ),
            pluginName: name,
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
    rawPlugins: RolldownPlugin[],
    outputOptions: OutputOptions,
  ): OutputOptions {
    const sortedPlugins = getSortedPlugins(
      'outputOptions',
      getObjectPlugins(rawPlugins),
    )

    for (const plugin of sortedPlugins) {
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

export function getObjectPlugins(plugins: RolldownPlugin[]): Plugin[] {
  return plugins.filter((plugin) => {
    if (!plugin) {
      return undefined
    }
    if ('_parallel' in plugin) {
      return undefined
    }
    if (plugin instanceof BuiltinPlugin) {
      return undefined
    }
    return plugin
  }) as Plugin[]
}

export function getSortedPlugins(
  hookName: 'options' | 'outputOptions' | 'onLog',
  plugins: readonly Plugin[],
): Plugin[] {
  const pre: Plugin[] = []
  const normal: Plugin[] = []
  const post: Plugin[] = []
  for (const plugin of plugins) {
    const hook = plugin[hookName]
    if (hook) {
      if (typeof hook === 'object') {
        if (hook.order === 'pre') {
          pre.push(plugin)
          continue
        }
        if (hook.order === 'post') {
          post.push(plugin)
          continue
        }
      }
      normal.push(plugin)
    }
  }
  return [...pre, ...normal, ...post]
}
