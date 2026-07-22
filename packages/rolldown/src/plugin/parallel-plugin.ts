import { pathToFileURL } from 'node:url';
import { assertRuntimeFeature } from '../runtime-support';

export type ParallelPlugin = {
  _parallel: {
    fileUrl: string;
    options: unknown;
  };
};

/** @internal */
export type DefineParallelPluginResult<Options> = (options: Options) => ParallelPlugin;

export function defineParallelPlugin<Options>(
  pluginPath: string,
): DefineParallelPluginResult<Options> {
  if (import.meta.browserBuild) {
    throw new Error('`defineParallelPlugin` is not supported in browser build');
  }
  // The runtime support matrix reports `parallelPlugins: false` for every
  // WASI artifact; fail at definition time, before a factory whose markers
  // would later spawn worker threads can escape.
  assertRuntimeFeature('parallelPlugins');
  return (options) => {
    return { _parallel: { fileUrl: pathToFileURL(pluginPath).href, options } };
  };
}
