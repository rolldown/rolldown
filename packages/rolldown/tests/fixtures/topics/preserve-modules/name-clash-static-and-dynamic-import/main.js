import value from './static.js';

export default value;
export const dynamic = import('./dynamic.js').then((m) => m.default);
