import { DEV, PROD } from './env.js';
import { dead, live } from './internal.js';

export function b() {
  if (DEV) dead();
  return PROD ? live() : dead();
}
