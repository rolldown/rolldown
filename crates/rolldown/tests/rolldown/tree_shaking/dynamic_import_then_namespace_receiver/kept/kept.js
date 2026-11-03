export function then(resolve) {
  globalThis.keptThenCalls = (globalThis.keptThenCalls ?? 0) + 1;
  globalThis.thenCalled();
  resolve({});
}
