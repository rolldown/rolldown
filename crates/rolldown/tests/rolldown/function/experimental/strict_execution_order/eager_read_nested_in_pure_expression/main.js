import './patch.js';
import { value } from './reader.js';

if (value !== 42) {
  throw new Error(`reader evaluated before patch: value = ${value}`);
}
