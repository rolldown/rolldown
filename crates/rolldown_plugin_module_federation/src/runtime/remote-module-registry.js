import { loadRemote, loadShare } from '@module-federation/runtime';

const registry = {};
const loading = {};

export async function loadRemoteToRegistry(id) {
  const promise = loading[id];
  if (promise) {
    await promise;
  } else {
    loading[id] = loadRemote(id);
    registry[id] = await loading[id];
  }
}

export async function loadSharedToRegistry(id) {
  const promise = loading[id];
  if (promise) {
    await promise;
  } else {
    loading[id] = loadShare(id);
    registry[id] = (await loading[id])();
  }
}

export function getModuleFromRegistry(id) {
  const module = registry[id];
  if (!module) {
    throw new Error(`Module ${id} not found in registry`);
  }
  return module;
}
