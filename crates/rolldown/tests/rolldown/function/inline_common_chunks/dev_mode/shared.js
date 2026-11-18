import { leaf } from './leaf.js';

export function shared() {
  return 'shared+' + leaf();
}
