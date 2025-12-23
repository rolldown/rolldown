// Module A: both imports and dynamically imports shared module
// This creates a scenario where shared.js is in the same chunk as moduleA
import { shared } from './shared.js';

// This dynamic import is ineffective because shared.js is already in this chunk
import('./shared.js').then(mod => {
  console.log('Module A dynamic import:', mod.shared);
});

export const foo = shared + ' from A';
