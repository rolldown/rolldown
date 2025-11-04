import { PluginFilterStorage } from './plugin-filter-storage';

/**
 * Global storage mapping plugin names to their filter storage.
 * This allows filter overrides set in the options hook to persist
 * and be applied when the full plugin context is created.
 */
export const pluginFilterStorageMap: Map<string, PluginFilterStorage> = new Map<string, PluginFilterStorage>();

export function getOrCreateFilterStorage(pluginName: string): PluginFilterStorage {
  let storage = pluginFilterStorageMap.get(pluginName);
  if (!storage) {
    storage = new PluginFilterStorage();
    pluginFilterStorageMap.set(pluginName, storage);
  }
  return storage;
}
