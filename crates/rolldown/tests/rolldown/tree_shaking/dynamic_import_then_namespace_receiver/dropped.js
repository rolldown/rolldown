export function then(resolve) {
  globalThis.droppedThenCalls = (globalThis.droppedThenCalls ?? 0) + 1;
  resolve({});
}
