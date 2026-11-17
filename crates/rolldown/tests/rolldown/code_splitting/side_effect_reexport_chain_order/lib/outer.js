import './before.js';
export { foo } from './barrel.js';
import './after.js';
console.log('outer');
