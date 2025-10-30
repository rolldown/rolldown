// Main entry
import { helper } from './lib.js';
console.log(helper());

// Also dynamically import another module that uses the same shared code
import('./dynamic.js');
