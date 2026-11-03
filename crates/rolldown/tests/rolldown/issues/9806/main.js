import nodeAssert from 'node:assert';
import { Other } from './pkg-barrel/index.mjs';

nodeAssert.equal(Other, 'other-value');
