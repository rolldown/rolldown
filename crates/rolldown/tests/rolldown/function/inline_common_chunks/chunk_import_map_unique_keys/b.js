import { foo } from './packages/foo/lib/index.js';
import { bar } from './packages/bar/lib/index.js';
globalThis.events.push('b ' + foo + ' ' + bar);
