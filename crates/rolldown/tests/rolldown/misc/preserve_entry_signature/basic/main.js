import { value as shared } from './lib.js';
import('./dynamic.js');

console.log(`shared: `, shared);

export const unused = shared;
