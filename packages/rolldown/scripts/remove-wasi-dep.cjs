// This script removes `@rolldown/binding-wasm32-wasi` from `optionalDependencies`
// after `napi pre-publish` adds it. The WASI binding package's `cpu: ["wasm32"]`
// constraint is not recognized by npm, causing it to be installed on all platforms
// and bringing in unnecessary transitive dependencies (@emnapi/*, @tybys/*).
// Users who need WASI support can install `@rolldown/binding-wasm32-wasi` manually.
'use strict';

const fs = require('node:fs');
const path = require('node:path');

const pkgPath = path.resolve(__dirname, '../package.json');
const pkg = JSON.parse(fs.readFileSync(pkgPath, 'utf-8'));

if (pkg.optionalDependencies) {
  delete pkg.optionalDependencies['@rolldown/binding-wasm32-wasi'];
}

fs.writeFileSync(pkgPath, JSON.stringify(pkg, null, 2) + '\n');
