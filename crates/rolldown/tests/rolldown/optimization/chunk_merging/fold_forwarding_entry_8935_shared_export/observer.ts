import { foo } from './index.ts';
import isOdd from './is-odd.js';

export function check(value: number) {
  return foo(value) === isOdd(value);
}
