// Never executed in this tab: reachable only through a dynamic import that never fires.
// The top-level push is the tripwire — HMR must not run this module.
import { value } from './common-child.js';
import { note } from './cold-only-dep.js';

window.__executed.push('parent-cold');
console.log(value, note);
