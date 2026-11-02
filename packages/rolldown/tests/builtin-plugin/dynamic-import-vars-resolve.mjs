import assert from 'node:assert/strict';
import path from 'node:path';
import { rolldown } from 'rolldown';
import { viteDynamicImportVarsPlugin } from 'rolldown/experimental';

const root = process.argv[2];
const bundle = await rolldown({
  input: path.join(root, 'main.js'),
  plugins: [
    viteDynamicImportVarsPlugin({
      async resolver(id) {
        return id.replace('@data/', `${path.join(root, 'data')}/`);
      },
    }),
  ],
});
const { output } = await bundle.generate();
assert.match(output[0].code, /import\.meta\.glob\("\.\/data\/\*\.js"\)/);
