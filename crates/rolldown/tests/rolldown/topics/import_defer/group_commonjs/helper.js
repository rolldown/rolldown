import d from './d.cjs';

globalThis.__events.push('helper');
export const helper = d + 1;
export function lazy() {
  return require('./lazy.cjs');
}
