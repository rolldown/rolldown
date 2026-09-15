import { depUrl } from './dep.js';

export const mainUrl = import.meta.ROLLUP_FILE_URL_REF;
console.log(depUrl, mainUrl);
