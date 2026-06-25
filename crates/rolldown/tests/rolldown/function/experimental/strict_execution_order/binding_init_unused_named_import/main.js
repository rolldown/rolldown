import { used, unused } from './barrel.js';

// Only `used` is read. `unused` is imported but never referenced.
console.log(used);
