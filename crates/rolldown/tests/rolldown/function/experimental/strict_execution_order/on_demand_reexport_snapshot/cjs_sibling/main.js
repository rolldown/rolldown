import { LOCAL } from './mixed.js';
import { mutate } from '../state.js';

mutate();
export const result = LOCAL();
export async function read() {
  return (await import('./lazy.js')).SNAPSHOT;
}
export async function readExtra() {
  return (await import('./extra.js')).EXTRA;
}
