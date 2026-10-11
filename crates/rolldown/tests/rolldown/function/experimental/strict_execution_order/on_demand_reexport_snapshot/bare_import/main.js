import './forwarder.js';
import { mutate } from '../state.js';

mutate();
export async function read() {
  return (await import('../lazy.js')).SNAPSHOT;
}
