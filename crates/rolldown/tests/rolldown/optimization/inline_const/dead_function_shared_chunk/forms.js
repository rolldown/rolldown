import { DEV } from './env.js';
import { throw_error } from './internal.js';
import * as internal from './internal.js';

export function forms() {
  DEV && throw_error('and', 'dead');
  !DEV || internal.throw_error('or', 'dead');
  DEV ?? internal.throw_error('nullish', 'dead');
  return DEV ? internal.bold : 'production';
}
