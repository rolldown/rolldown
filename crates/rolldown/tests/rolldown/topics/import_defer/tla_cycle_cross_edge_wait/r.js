import { log } from './log.js';
import { v as v_p } from './p.js';
import { v as v_x } from './x.js';

export var v;
log.push('r start p=' + v_p + ' x=' + v_x);
v = 'set';
log.push('r end');
