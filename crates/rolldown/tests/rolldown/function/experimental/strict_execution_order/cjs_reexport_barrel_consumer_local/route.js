import { Stack, cloneDeep } from 'pure-barrel';

const stack = new Stack();
stack.push(cloneDeep(1));
globalThis.__events.push('route');

export const value = stack.items[0];
