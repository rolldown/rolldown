// An escape and a static access.
import defer * as ns from './x.cjs';

export const getB = () => ns;
export const selfB = () => ns.self();
