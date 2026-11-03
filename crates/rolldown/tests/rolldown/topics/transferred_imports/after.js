export function after() {
  globalThis.__events.push('after-anchor');
  return globalThis.__events.length;
}
