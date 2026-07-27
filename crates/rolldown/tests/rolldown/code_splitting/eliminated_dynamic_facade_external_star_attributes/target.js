import { read } from './reader.js';

export const x = 1;
read();

export * from 'external' with { type: 'json' };
