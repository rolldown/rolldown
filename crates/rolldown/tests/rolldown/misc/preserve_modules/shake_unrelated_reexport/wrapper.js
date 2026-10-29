import { bar } from './a.js';

console.log('wrapper side effect');
console.log(bar());

export { foo } from './a.js';
