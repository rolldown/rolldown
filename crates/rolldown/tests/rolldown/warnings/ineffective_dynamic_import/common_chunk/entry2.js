// Entry 2: also imports lib statically
// lib.js should become a common chunk shared between entry1 and entry2
import { lib } from './lib.js';
console.log('Entry 2:', lib);
