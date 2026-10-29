import * as ns from './s1.js';
import { tag } from './s3.js';
globalThis.events.push(
  'B ' + ns.one + ' ' + tag`b${ns.one}` + ' ' + ns.tagged + ' ' + new ns.C().k,
);
