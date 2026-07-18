import type { ParallelPlugin } from '../plugin/parallel-plugin';

function getOwnDataProperty(object: object, key: PropertyKey): PropertyDescriptor | undefined {
  try {
    const descriptor = Object.getOwnPropertyDescriptor(object, key);
    return descriptor && 'value' in descriptor ? descriptor : undefined;
  } catch {
    return undefined;
  }
}

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
