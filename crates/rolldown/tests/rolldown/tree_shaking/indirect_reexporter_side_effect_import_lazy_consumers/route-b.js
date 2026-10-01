import { readConfigured, other } from './library/index.js';

export const b = () => `b:${readConfigured()}:${other}`;
