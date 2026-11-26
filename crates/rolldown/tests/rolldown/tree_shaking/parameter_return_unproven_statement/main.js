import { disabled, identity, quiet } from './lib.js';

if (disabled) identity(1);
else quiet();
console.log('ready');
