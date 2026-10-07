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

/** @internal */
export function assertParallelPluginsSupported(): void {
  assertRuntimeFeature('parallelPlugins');
}

export function defineParallelPlugin<Options>(
  pluginPath: string,
): DefineParallelPluginResult<Options> {
  if (import.meta.browserBuild) {
    assertParallelPluginsSupported();
    throw new Error('Parallel plugins unexpectedly reported support in a browser build');
  }
  assertParallelPluginsSupported();
  return (options) => {
    return { _parallel: { fileUrl: pathToFileURL(pluginPath).href, options } };
  };
}
