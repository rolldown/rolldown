import { lightValueB } from './library/index.js';

globalThis.fixtureLog.push(`consumer-light-b:${lightValueB()}`);

export const value = lightValueB();
