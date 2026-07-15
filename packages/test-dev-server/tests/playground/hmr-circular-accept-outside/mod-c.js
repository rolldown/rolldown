import { value as _value } from './mod-a.js';

// Reading `_value` here may throw (TDZ) or read a placeholder depending on how the
// bundle evaluates the cycle; the assertion only cares that the chain renders and that
// an edit to mod-b flows through the circle to the accepting boundary in main.js.
let __value;
try {
  __value = `${_value}`;
} catch {
  __value = 'mod-a (cycle placeholder)';
}

export const value = `mod-c -> ${__value}`;
