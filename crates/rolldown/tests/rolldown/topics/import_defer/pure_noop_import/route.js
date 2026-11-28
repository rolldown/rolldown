import { isObject } from './is-object.js';

export function check() {
  return [isObject({}), isObject(1)];
}
