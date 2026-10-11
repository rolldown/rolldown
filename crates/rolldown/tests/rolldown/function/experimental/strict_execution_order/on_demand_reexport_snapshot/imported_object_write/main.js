import './barrel.js';
import { mutate } from './state.js';

mutate();

export async function read() {
  return (await import('./leaf.js')).readValue();
}
