import nodeFs from "node:fs";
import nodeAssert from 'node:assert';
import nodePath from 'node:path';

nodeAssert(!nodeFs.readFileSync(nodePath.join(import.meta.dirname, 'dist/main.js'), 'utf-8').includes('foo()'), "foo() should be tree-shaked");
