// @__NO_SIDE_EFFECTS__
export function wrapper(load) {
  return {
    load,
  };
}

export const pages = [wrapper(() => import('./page.js'))];
import { noop } from './noop.js';

noop(() => import('./unused.js'));
