import { log } from './log.js';
import { v as v_c } from './c.js';

export var v;
log.push('p start c=' + v_c);
v = 'set';
log.push('p end');
