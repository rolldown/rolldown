import { getSharedB } from './common-a.js';

const value = getSharedB();
console.log(`dynamic1:${value}`);

export { value };
