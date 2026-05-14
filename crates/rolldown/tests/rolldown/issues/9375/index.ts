import { createPinia } from 'pinia';
var freeModule = freeExports && typeof module == 'object' && module && !module.nodeType && module;
var pinia = createPinia();
console.log([pinia, freeModule]);
