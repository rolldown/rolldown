import { log } from './log.js';
import defer * as ns from './x.js';

const use = (value) => value;
export const descriptor = Object.getOwnPropertyDescriptor(use(ns), 'then');
export const hasThen = 'then' in use(ns);
export const before = [...log];
// `Reflect.ownKeys` evaluates the module, and the keys do not contain `then`.
export const keys = Reflect.ownKeys(use(ns)).map(String);
export const after = [...log];
