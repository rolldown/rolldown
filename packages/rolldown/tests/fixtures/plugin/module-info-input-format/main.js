import { foo } from './esm.js';
import './no-type-pkg/unknown.js';
import '\0virtual:unknown.js';
import 'data:text/javascript,console.log("data url")';
const cjs = require('./cjs.js');
import('./side-effect.js');

export const result = [foo, cjs];
