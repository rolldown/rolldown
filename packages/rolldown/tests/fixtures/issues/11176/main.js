import { save as a } from './replaced.js';
import { save as b } from './earlier-then-replaced.js';
import { save as c } from './earlier-then-omitted.js';

globalThis.actions = [a, b, c];
