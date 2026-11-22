import a from './a.cjs';
import b from './b.cjs';

globalThis.__events.push('route');
export function read() {
  return [a, b];
}
