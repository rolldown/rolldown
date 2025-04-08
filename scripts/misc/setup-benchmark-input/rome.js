import glob from 'fast-glob';
import fsExtra from 'fs-extra';
// setup rome

if (fsExtra.existsSync('./tmp/bench/rome')) {
  console.log('[skip] setup rome already');
} else {
  console.log('Setup `rome` in tmp/bench');

  fsExtra.copySync('./tmp/github/rome/packages', './tmp/bench/rome/src/', {
    filter(src) {
      // an error happens on windows without this filter
      return !src.includes('.bin');
    },
  });
  fsExtra.writeFileSync(
    './tmp/bench/rome/src/entry.ts',
    'import "rome/bin/rome"',
  );
  fsExtra.writeFileSync(
    './tmp/bench/rome/src/tsconfig.json',
    JSON.stringify(
      {
        compilerOptions: {
          sourceMap: true,
          esModuleInterop: true,
          resolveJsonModule: true,
          moduleResolution: 'node',
          target: 'es2019',
          module: 'commonjs',
          baseUrl: '.',
        },
      },
      null,
      2,
    ),
  );

  const files = await glob('./tmp/bench/rome/src/**/*.{ts,tsx}');
  const problematicExportDefaultRE = /export default function \w+\(/;
  const importEqualRE = /import (\w+) = require\('(.+)'\)/;
  for (const file of files) {
    let content = await fsExtra.readFile(file, 'utf8');
    let hasReplaced = false;

    // Replace "import fs = require('fs')" with "const fs = require('fs')"
    // https://github.com/evanw/esbuild/blob/fc37c2fa9de2ad77476a6d4a8f1516196b90187e/Makefile#L1007-L1009
    if (importEqualRE.test(content)) {
      hasReplaced = true;
      content = content.replaceAll(
        /import (\w+) = require\('(.+)'\)/g,
        "const $1 = require('$2')",
      );
      console.log('Fixing import equal in', file);
    }

    // replace `export default function name()` with `export default function()`
    // rome uses a same identifier as a type and a value and that chokes babel
    if (problematicExportDefaultRE.test(content)) {
      hasReplaced = true;
      content = content.replace(
        problematicExportDefaultRE,
        'export default function(',
      );
    }

    if (hasReplaced) {
      await fsExtra.writeFile(file, content);
    }
  }
  // also replace some additional things in `@romejs/js-formatter/node/parentheses.ts`
  {
    const file =
      './tmp/bench/rome/src/@romejs/js-formatter/node/parentheses.ts';
    const content = await fsExtra.readFile(file, 'utf8');
    const newContent = content.replace(
      /import \{((?:.|\n)*)\} from '@romejs\/js-ast';/,
      "import type {$1} from '@romejs/js-ast';",
    );
    await fsExtra.writeFile(file, newContent);
  }
}
