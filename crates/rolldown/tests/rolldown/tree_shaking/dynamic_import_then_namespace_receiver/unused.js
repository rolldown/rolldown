export function then(resolve) {
  globalThis.thenCalls = (globalThis.thenCalls ?? 0) + 1;
  globalThis.thenCalled();
  resolve({});
}
