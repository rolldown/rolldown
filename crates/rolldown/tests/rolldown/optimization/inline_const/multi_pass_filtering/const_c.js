// Depends on CONST_B - discovered in pass 2 of cross_module_optimization
// This is where the filtering optimization kicks in
import { CONST_B } from './const_b.js';
export const CONST_C = CONST_B === 'foo';
