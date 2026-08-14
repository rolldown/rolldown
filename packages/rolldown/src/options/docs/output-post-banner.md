#### Examples

##### Adding build info

```js
import pkg from './package.json' with { type: 'json' };
import { execSync } from 'node:child_process';

const gitHash = execSync('git rev-parse --short HEAD').toString().trim();

export default {
  output: {
    minify: true,
    postBanner: `/* ${pkg.name}@${pkg.version} (${gitHash}) */`,
  },
};
```
