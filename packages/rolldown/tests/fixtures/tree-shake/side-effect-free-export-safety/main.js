import defaultFn from './default.js';
import './barrel-b.js';
import './cycle-entry.js';
import './default-cycle-a.js';
import './pre-init.js';
import { maybeFn } from './conditional.js';
import { evalReassigned } from './eval.js';
import { reassigned } from './reassigned.js';
import { restDefault } from './rest.js';

defaultFn(() => import('./unused-default.js'));
globalThis.reassignedResult = reassigned();
globalThis.conditionalBinding = maybeFn;
evalReassigned();
restDefault();
