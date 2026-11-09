import { DEV } from './dev.js';
import * as w from './warnings.js';

export function shared() {
  if (DEV) w.used();
  return 'shared';
}
