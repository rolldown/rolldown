import type { ParallelPlugin } from '../plugin/parallel-plugin';

/**
 * Returns the `_parallel` marker of a parallel plugin, or `undefined` if the
 * given plugin is not one.
 *
 * Detection is descriptor-based instead of using the `in` operator: only an
 * own data property named `_parallel` with the expected shape counts. This
 * avoids false positives from inherited or accessor `_parallel` properties on
 * regular plugins, and guarantees that callers can safely read `fileUrl` and
 * `options` from the returned value.
 */
export function getParallelPluginInfo(plugin: unknown): ParallelPlugin['_parallel'] | undefined {
  if (plugin === null || typeof plugin !== 'object') {
    return undefined;
  }
  const descriptor = Object.getOwnPropertyDescriptor(plugin, '_parallel');
  if (!descriptor || !('value' in descriptor)) {
    return undefined;
  }
  const parallel: unknown = descriptor.value;
  if (
    parallel === null ||
    typeof parallel !== 'object' ||
    typeof (parallel as { fileUrl?: unknown }).fileUrl !== 'string'
  ) {
    return undefined;
  }
  return parallel as ParallelPlugin['_parallel'];
}
