import type { ParallelPlugin } from '../plugin/parallel-plugin';
import { getOwnDataProperty } from './prototype-chain';

export function getParallelPluginInfo(plugin: unknown): ParallelPlugin['_parallel'] | undefined {
  if (plugin === null || (typeof plugin !== 'object' && typeof plugin !== 'function')) {
    return undefined;
  }
  const parallel = getOwnDataProperty(plugin, '_parallel')?.value;
  if (parallel === null || typeof parallel !== 'object') {
    return undefined;
  }
  const fileUrl = getOwnDataProperty(parallel, 'fileUrl');
  const options = getOwnDataProperty(parallel, 'options');
  if (!fileUrl || typeof fileUrl.value !== 'string' || !options) {
    return undefined;
  }
  return { fileUrl: fileUrl.value, options: options.value };
}
