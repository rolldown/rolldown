// Depends on CONST_A - discovered in pass 1 of cross_module_optimization
import { CONST_A } from './const_a.js';
export const CONST_B = CONST_A ? 'foo' : 'bar';
