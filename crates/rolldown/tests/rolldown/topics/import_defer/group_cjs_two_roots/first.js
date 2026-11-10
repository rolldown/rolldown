import a from './a.cjs';
import shared from './shared.cjs';

globalThis.__events.push('first');
export function read() {
  return [a, shared];
}
