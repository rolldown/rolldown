import type { MaybePromise } from '../types/utils'
import type { Plugin } from './index'

export type ThreadSafePluginImplementation = Plugin

export type Context = {
  /**
   * Thread number
   */
  threadNumber: number
}

export function defineThreadSafePluginImplementation<Options>(
  plugin: (
    Options: Options,
    context: Context,
  ) => MaybePromise<ThreadSafePluginImplementation>,
) {
  return plugin
}
