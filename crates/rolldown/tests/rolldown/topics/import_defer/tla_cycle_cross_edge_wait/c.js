import { log } from './log.js';
import { v as v_r } from './r.js';
import { v as v_t } from './t.js';

export var v;
log.push('c start r=' + v_r + ' t=' + v_t);
v = 'set';
log.push('c end');
