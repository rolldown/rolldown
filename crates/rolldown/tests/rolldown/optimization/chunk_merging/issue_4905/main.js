import { strictEqual } from 'node:assert';
import { foo } from './foo.js';

strictEqual(foo, 'foo');

import('./foo.js').then((mod) => {
  strictEqual(mod.foo, 'foo');
});
