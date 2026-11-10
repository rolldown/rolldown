import shared from './shared.cjs';
import c from './c.cjs';
import { helper, lazy } from './helper.js';

globalThis.__events.push('route');
export function read() {
  return [shared, c, helper];
}
export { lazy };
