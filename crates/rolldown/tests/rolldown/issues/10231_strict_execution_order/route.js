import { Stack, cloneDeep } from './barrel/index.js';

const stack = new Stack();
stack.push(cloneDeep({ value: 1 }));

export const value = stack.items[0];
