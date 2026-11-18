import { replaced } from './eval.js';
import { redeclared } from './redeclared.js';
import { restDefault } from './rest-default.js';
import { iterable } from './iterable.js';

replaced();
redeclared();
restDefault();
const [first] = iterable('destructuring');
iterable('removed: nothing runs on the result');
