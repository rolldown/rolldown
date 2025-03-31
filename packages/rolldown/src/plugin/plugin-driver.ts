import { LOG_LEVEL_INFO, type LogLevelOption } from '../log/logging'
import { Plugin } from './'
import { normalizeHook } from '../utils/normalize-hook'
import { InputOptions, OutputOptions, RolldownPlugin } from '..'
import { getLogger, getOnLog } from '../log/logger'
import { BuiltinPlugin } from '../builtin-plugin/constructors'
import { normalizePluginOption } from '../utils/normalize-plugin-option'
import { MinimalPluginContextImpl } from './minimal-plugin-context'
import type { LogHandler } from '../types/misc'

export class PluginDriver {
  public static async callOptionsHook(
    inputOptions: InputOptions,
    watchMode: boolean = false,
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
      watchMode,
    )

    for (const plugin of plugins) {
      const name = plugin.name || 'unknown'
      const options = plugin.options
      if (options) {
        const { handler } = normalizeHook(options)
        const result = await handler.call(
          new MinimalPluginContextImpl(
            logger,
            logLevel,
            name,
            watchMode,
            'onLog',
          ),
          inputOptions,
        )

        if (result) {
          inputOptions = result
        }
      }
    }

    return inputOptions
  }

  public static callOutputOptionsHook(
    rawPlugins: RolldownPlugin[],
    outputOptions: OutputOptions,
    onLog: LogHandler,
    logLevel: LogLevelOption,
    watchMode: boolean,
  ): OutputOptions {
    const sortedPlugins = getSortedPlugins(
      'outputOptions',
      getObjectPlugins(rawPlugins),
    )

    for (const plugin of sortedPlugins) {
      const name = plugin.name || 'unknown'
      const options = plugin.outputOptions
      if (options) {
        const { handler } = normalizeHook(options)
        const result = handler.call(
          new MinimalPluginContextImpl(onLog, logLevel, name, watchMode),
          outputOptions,
        )

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
