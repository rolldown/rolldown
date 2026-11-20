import * as ns from './lib.js';
import { kept, replacedEmpty, replacedFunction, replacedLet, replacedVar } from './lib.js';
import './local.js';

replacedLet();
replacedVar();
replacedFunction();
replacedEmpty();
ns.replacedLet();
kept();
