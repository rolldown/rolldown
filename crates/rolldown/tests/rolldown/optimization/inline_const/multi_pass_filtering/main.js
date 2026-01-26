// Entry point that uses the constant chain
import { CONST_C } from './const_c.js';
import { unrelatedFn } from './unrelated.js';

// This should be eliminated when CONST_C is known to be true
if (!CONST_C) {
  console.log('this should be eliminated');
}

// This should remain
console.log(unrelatedFn());
