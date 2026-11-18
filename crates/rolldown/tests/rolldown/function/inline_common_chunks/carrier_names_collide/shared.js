import { helper } from './vendor.js';
export let count = 0;
export function bump() {
  count += 1;
  return helper() + count;
}
export const marker = { tag: 'shared' };
