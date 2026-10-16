import { bar } from './packages/bar/lib/index.js';
import { baz } from './packages/baz/lib/index.js';
globalThis.events.push('c ' + bar + ' ' + baz);
