import { external } from 'external';
import { foo, bar as abc } from  "./cjs.js";
import "./commonjs.mjs";
console.log(external, foo, abc)