import { leaf } from './leaf.js';

export function shared(name) {
  return `${name}:${leaf()}`;
}
