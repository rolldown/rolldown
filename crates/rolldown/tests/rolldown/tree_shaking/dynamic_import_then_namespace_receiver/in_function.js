export function then(resolve) {
  globalThis.inFunctionThenCalls = (globalThis.inFunctionThenCalls ?? 0) + 1;
  resolve({});
}
