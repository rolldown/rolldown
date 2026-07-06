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

/**
 * Reject already-materialized descriptors without awaiting any neighboring
 * plugin promises. See internal-docs/async-runtime/implementation.md.
 *
 * @internal
 */
export function assertParallelPluginOptionsSupported(...pluginOptions: unknown[]): void {
  const pending = [...pluginOptions];
  const visitedArrays = new Set<unknown[]>();
  while (pending.length > 0) {
    const value = pending.pop();
    if (Array.isArray(value)) {
      if (visitedArrays.has(value)) continue;
      visitedArrays.add(value);
      const length = value.length;
      for (let index = length - 1; index >= 0; index -= 1) {
        pending.push(value[index]);
      }
      continue;
    }
    if (
      value !== null &&
      (typeof value === 'object' || typeof value === 'function') &&
      '_parallel' in value
    ) {
      assertParallelPluginsSupported();
      return;
    }
  }
}

export function defineParallelPlugin<Options>(
  pluginPath: string,
): DefineParallelPluginResult<Options> {
  assertParallelPluginsSupported();
  return (options) => {
    return { _parallel: { fileUrl: pathToFileURL(pluginPath).href, options } };
  };
}
