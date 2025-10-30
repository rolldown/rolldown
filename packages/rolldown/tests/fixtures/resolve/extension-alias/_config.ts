import path from 'node:path';
import { defineTest } from 'rolldown-tests';
const entry = path.join(__dirname, './main.ts');

export default defineTest({
  config: {
    input: entry,
    resolve: {
      extensionAlias: { '.js': ['.ts', '.js'] },
    },
  },
});
