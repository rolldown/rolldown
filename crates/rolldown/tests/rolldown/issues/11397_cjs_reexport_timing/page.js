import data from './a.cjs';
import { value } from './target.js';

globalThis.before = value;
data.value = 9;
