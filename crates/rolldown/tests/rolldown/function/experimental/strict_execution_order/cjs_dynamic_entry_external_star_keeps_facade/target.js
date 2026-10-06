import { read } from './reader.js';

globalThis.__externalStarLog.push('target');

export const x = 1;
read();

export * from 'external';
export * from 'external';
export * from 'primitive';
