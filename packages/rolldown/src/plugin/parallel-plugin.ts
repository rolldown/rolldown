import { pathToFileURL } from 'node:url';
import { assertRuntimeFeature } from '../runtime-support';
import { getParallelPluginInfo } from '../utils/parallel-plugin';

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
 * Reject descriptors reachable through already-materialized own data
 * properties without executing plugin-array accessors or indexed proxy gets.
 * See internal-docs/async-runtime/implementation.md.
 *
 * @internal
 */
export function assertParallelPluginOptionsSupported(...pluginOptions: unknown[]): void {
  const pending = [...pluginOptions];
  const visitedArrays = new Set<unknown[]>();
  while (pending.length > 0) {
    const value = pending.pop();
    if (isArray(value)) {
      if (visitedArrays.has(value)) continue;
      visitedArrays.add(value);
      enqueueOwnArrayDataProperties(value, pending);
      continue;
    }
    if (getParallelPluginInfo(value)) {
      assertParallelPluginsSupported();
      return;
    }
  }
}

function isArray(value: unknown): value is unknown[] {
  try {
    return Array.isArray(value);
  } catch {
    return false;
  }
}

function enqueueOwnArrayDataProperties(value: unknown[], pending: unknown[]): void {
  const length = getOwnDataProperty(value, 'length')?.value;
  if (typeof length !== 'number' || !Number.isSafeInteger(length) || length < 0) return;

  let keys: (string | symbol)[];
  try {
    keys = Reflect.ownKeys(value);
  } catch {
    return;
  }

  const entries: { index: number; value: unknown }[] = [];
  for (const key of keys) {
    const index = getArrayIndex(key);
    if (index === undefined || index >= length) continue;
    const descriptor = getOwnDataProperty(value, key);
    if (descriptor) {
      entries.push({ index, value: descriptor.value });
    }
  }
  entries.sort((left, right) => right.index - left.index);
  for (const entry of entries) {
    pending.push(entry.value);
  }
}

function getOwnDataProperty(value: object, key: PropertyKey): PropertyDescriptor | undefined {
  try {
    const descriptor = Object.getOwnPropertyDescriptor(value, key);
    return descriptor && 'value' in descriptor ? descriptor : undefined;
  } catch {
    return undefined;
  }
}

function getArrayIndex(key: PropertyKey): number | undefined {
  if (typeof key !== 'string' || key === '') return;
  const index = Number(key);
  if (!Number.isInteger(index) || index < 0 || index >= 0xffff_ffff || String(index) !== key) {
    return;
  }
  return index;
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
