import './a.js';
import { b } from './base.js';
(globalThis.order ??= []).push('main' + b);
