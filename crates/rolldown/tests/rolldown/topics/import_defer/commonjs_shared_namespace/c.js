// Two deferred imports of one module in one file. Another chunk imports both objects.
import defer * as one from './x.cjs';
import defer * as two from './x.cjs';

export const getC1 = () => one;
export const getC2 = () => two;
export { one, two };
