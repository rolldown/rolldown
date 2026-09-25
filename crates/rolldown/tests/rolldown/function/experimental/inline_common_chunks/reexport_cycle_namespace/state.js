import { format } from './format.js';

export let count = 0;
export function bump() {
  count++;
  return format();
}
