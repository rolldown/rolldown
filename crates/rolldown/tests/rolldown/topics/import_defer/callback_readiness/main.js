import defer * as target from './target.js';
import defer * as other from './other.js';
import defer * as dep from './dep.js';

globalThis.events = [];
globalThis.read = () => target.x;
globalThis.readOther = () => other.x;
export const depValue = dep.x;
export const targetValue = target.x;
export const again = target.x;
