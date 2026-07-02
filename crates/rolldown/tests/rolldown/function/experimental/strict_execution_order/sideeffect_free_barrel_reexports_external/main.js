import nodeAssert from 'node:assert';
import { sep } from './barrel';

nodeAssert.equal(typeof sep, 'string');
