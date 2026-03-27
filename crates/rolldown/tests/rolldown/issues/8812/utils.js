import fakeCore from './fake-core.cjs';
export function useCore() {
  return fakeCore.registry;
}
