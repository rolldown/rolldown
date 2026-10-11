import { LOCAL } from './mixed.js';

globalThis.marker = 'after';
export const result = LOCAL;
export async function read() {
  return (await import('./lazy.js')).SNAPSHOT;
}
