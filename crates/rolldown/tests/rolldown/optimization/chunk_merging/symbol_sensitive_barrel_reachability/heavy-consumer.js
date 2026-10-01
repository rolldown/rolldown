import { heavyValue } from './library/index.js';

globalThis.fixtureLog.push(`consumer-heavy:${heavyValue()}`);

export const value = heavyValue();
