import b from './b.cjs';
import shared from './shared.cjs';

globalThis.__events.push('second');
export function read() {
  return [b, shared];
}
