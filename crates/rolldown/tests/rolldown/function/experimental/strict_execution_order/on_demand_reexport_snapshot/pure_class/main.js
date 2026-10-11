import { LOCAL } from './mixed.js';
import { mutate } from '../state.js';

mutate();
export const result = LOCAL;
export async function read() {
  return (await import('./lazy.js')).SNAPSHOT.value;
}
