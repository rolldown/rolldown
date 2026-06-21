import { value } from './dep.js';

export let libValue;

try {
  libValue = value + '+lib';
} catch {}
