import { d as fromNodeShim } from './shim-node.mjs';
import { d as fromNonNodeShim } from './shim-non-node.js';

export function readNode() {
  return fromNodeShim;
}

export function readNonNode() {
  return fromNonNodeShim;
}
