import { careful } from './warnings.js';

export function b(name) {
  if (!name) careful({ name: 'b' });
  return name;
}
