import { DEV } from './env.js';
import { bold } from './internal.js';

export function careful(values) {
  if (DEV) {
    console.warn(`%c careful ${values.name}`, bold);
  } else {
    console.warn('https://example.com/careful');
  }
}
