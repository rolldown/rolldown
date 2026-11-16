import { header } from './header.js';
(globalThis.log ??= []).push('page');
export default () => header;
