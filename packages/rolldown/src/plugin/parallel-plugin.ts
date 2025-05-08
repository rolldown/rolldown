import { pathToFileURL } from 'node:url';

export type ParallelPlugin = {
  /** @internal */
  _parallel: {
    fileUrl: string;
    options: unknown;
  };
};

export type DefineParallelPluginResult<Options> = (
  options: Options,
) => ParallelPlugin;

export function defineParallelPlugin<Options>(
  pluginPath: string,
): DefineParallelPluginResult<Options> {
  if (import.meta.browserBuild) {
    throw new Error('`defineParallelPlugin` is not supported in browser build');
  }
  return (options) => {
    return { _parallel: { fileUrl: pathToFileURL(pluginPath).href, options } };
  };
}
