import { lightValueA } from './library/index.js';

globalThis.fixtureLog.push(`consumer-light-a:${lightValueA()}`);

export const value = lightValueA();
