// Nothing reads the namespace, so this entry never runs `x.cjs`.
import defer * as x from './x.cjs';

export const name = 'deferred';
