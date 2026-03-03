import * as fs from 'node:fs';
import { defineTest } from 'rolldown-tests';

export default defineTest({
  config: {
    input: './main.jsx',
    plugins: [
      {
        name: 'test-plugin',
        load: function (id) {
          let code = fs.readFileSync(id).toString();
          return {
            code,
          };
        },
      },
    ],
    external: ['react/jsx-runtime'],
  },
});
