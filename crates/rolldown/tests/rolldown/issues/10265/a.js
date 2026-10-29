import { getB } from './b.js';
import { leaf } from './leaf.js';

export function getA() {
  return getB && leaf ? 'a' : 'unreachable';
}
