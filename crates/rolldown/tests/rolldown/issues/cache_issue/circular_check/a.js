export const before = 'a-before';

const b = require('./b.js');

export const seenB = b.b;
export const after = 'a-after';
