import { v as value2 } from './generated-dep2.js';

console.log('main2', value2);
import('./generated-dynamic2.js');

export { value2 };
