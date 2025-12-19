// This should trigger an ineffective dynamic import warning
// because foo.js is both statically imported and dynamically imported
// and will end up in the same chunk
import { foo } from './foo.js';
import('./foo.js').then(mod => {
  console.log('Dynamic import:', mod.foo);
});
console.log('Static import:', foo);
