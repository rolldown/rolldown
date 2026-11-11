import { DEV } from './env.js';
import { throw_error } from './internal.js';

export function oops(values) {
  if (DEV) {
    throw_error('oops', `Something went wrong with ${values.name}`);
  }
  throw new Error('https://example.com/oops');
}
