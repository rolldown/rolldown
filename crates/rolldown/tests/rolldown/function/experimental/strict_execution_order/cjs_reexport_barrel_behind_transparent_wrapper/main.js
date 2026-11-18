import { cloneDeep, helper } from './forwarder.js';

globalThis.__result = [cloneDeep({ a: 1 }), helper()];
