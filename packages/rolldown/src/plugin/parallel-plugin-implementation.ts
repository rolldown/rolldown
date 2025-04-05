import type { MaybePromise } from '../types/utils';
import type { Plugin } from './index';

export type ParallelPluginImplementation = Plugin;

export type Context = {
  /**
   * Thread number
   */
  threadNumber: number;
};

export function defineParallelPluginImplementation<Options>(
  plugin: (
    Options: Options,
    context: Context,
  ) => MaybePromise<ParallelPluginImplementation>,
): (
  Options: Options,
  context: Context,
) => MaybePromise<ParallelPluginImplementation> {
  return plugin;
}
