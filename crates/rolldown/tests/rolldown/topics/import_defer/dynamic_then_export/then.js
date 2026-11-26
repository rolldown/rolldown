export function resolveImport(resolve) {
  globalThis.events.push('then');
  resolve('ordinary');
}
