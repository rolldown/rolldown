import data from './a.cjs';
import * as target from './target.mjs';

globalThis.before = target.count;
globalThis.data = data;
