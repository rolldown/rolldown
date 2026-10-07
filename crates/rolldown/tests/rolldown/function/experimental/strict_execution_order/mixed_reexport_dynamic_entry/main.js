import { LOCAL } from './shared.js';

export async function read() {
  return [LOCAL, (await import('./shared.js')).A.value, (await import('./lazy.js')).read()];
}
