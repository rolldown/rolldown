import { snapshot } from './common.js';
export function log(value) {
  return `${value} ${snapshot}`;
}
