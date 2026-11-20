// A chunk that imports the two deferred namespace objects of `c.js` from the main chunk.
import { one, two } from './main.js';

export const getOne = () => one;
export const getTwo = () => two;
