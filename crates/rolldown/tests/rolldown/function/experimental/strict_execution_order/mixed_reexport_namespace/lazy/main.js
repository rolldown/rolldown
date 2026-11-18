import { LOCAL } from '../mixed.js';

export const result = LOCAL;
export async function read() {
  return Object.entries(await import('./lazy.js'));
}
