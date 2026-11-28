// Nothing reads either namespace, so the entry runs neither module.
import defer * as x from './x.cjs';
import defer * as y from './y.cjs';

export const name = 'main';
