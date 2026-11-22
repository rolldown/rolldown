import { fooA, getA } from './a.js';
import { getB } from './b.js';
import { getC } from './c.js';
import { getM } from './m.mjs';
import defer * as wrapped from './wrapped.js';

const log = (globalThis.__extSharedLog ??= []);
export const before = [...log];
export const results = async () => {
  const [one, two] = getC();
  const dynamic = await import.defer('ext-shared');
  return {
    twoFiles: getA() === getB(),
    oneFile: one === two && one === getA(),
    wrapper: wrapped.getW() === getA(),
    dynamic: dynamic === getA(),
    foo: fooA(),
    defaultTypes: [typeof getA().default, typeof getM().default],
    log: [...log],
  };
};
export { getA };
