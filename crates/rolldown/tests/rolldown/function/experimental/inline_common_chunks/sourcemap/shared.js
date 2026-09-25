globalThis.mappedInitializations = (globalThis.mappedInitializations ?? 0) + 1;

export let mappedValue = 42;
export function readMappedValue() {
  return mappedValue;
}
