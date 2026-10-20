import lib from './lib.js';
import { getShared } from './shared.js';

globalThis.mainResult = { value: lib.value, shared: getShared() };
