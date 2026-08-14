import shallow from './root.txt';
import deep from './a/b/c/deep.txt';
const shallowReq = require('./root.txt');
const deepReq = require('./a/b/c/deep.txt');
console.log(shallow, deep, shallowReq, deepReq);
