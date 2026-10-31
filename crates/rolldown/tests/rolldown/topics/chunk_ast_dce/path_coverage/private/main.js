import { First } from './first.js';
import { Second } from './second.js';

globalThis.__pathEvents.push('private-main-anchor');
const second = new Second();
const Inner = second.inner();
export const result = [new First().read(), second.read(), new Inner().read()];
