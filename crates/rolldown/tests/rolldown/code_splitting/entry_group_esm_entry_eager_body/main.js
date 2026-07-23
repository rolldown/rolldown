import leaf from './leaf.js';
import { value } from './leaf-esm.js';
if (leaf.ok !== true) {
  throw new Error(`cjs leaf not initialized: ${JSON.stringify(leaf)}`);
}
if (value !== 42) {
  throw new Error(`esm leaf value wrong: ${value}`);
}
export const done = true;
