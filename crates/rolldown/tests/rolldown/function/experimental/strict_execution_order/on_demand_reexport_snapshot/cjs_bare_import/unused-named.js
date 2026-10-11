import { SNAPSHOT } from './forwarder.js';
import { mutate } from './state.cjs';

mutate();
export async function read() {
  return (await import('./lazy.js')).SNAPSHOT;
}
