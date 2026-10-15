import { foo } from './packages/foo/lib/index.js';
import { baz } from './packages/baz/lib/index.js';
globalThis.events.push('a ' + foo + ' ' + baz);
