import { ns } from './barrel.js';

// An access through a re-export reads the deferred namespace object of `barrel.js`.
export const readFoo = () => ns.foo;
