import { value } from './shared.js';
import { a, s } from './values.js';

globalThis.log = [];
const log = (msg) => globalThis.log.push(msg);

// Arithmetic, bitwise, and string comparison operators on imported literals.
export function main() {
  if (a - 1) log('DEAD_subtract');
  if (a * 0) log('DEAD_multiply');
  if (a / 2 - 0.5) log('DEAD_divide');
  if (a % 1) log('DEAD_remainder');
  if (a ** 0 - 1) log('DEAD_exponent');
  if (a & 0) log('DEAD_bit_and');
  if (a ^ 1) log('DEAD_bit_xor');
  if (s < 'a') log('DEAD_string_less');
  if (s > 'z') log('DEAD_string_greater');
  if (value - 1) log('DEAD_shared_chunk');
  if (same) log('DEAD_conditional_same');
  return 1;
}

// An unknown condition with equal alternatives.
const same = globalThis.unknown ? false : false;

// An arithmetic result decides a dynamic import.
export async function load() {
  if (value - 1) await import('./dead.js');
  return 1;
}
