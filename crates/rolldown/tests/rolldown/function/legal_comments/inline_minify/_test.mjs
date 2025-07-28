
import nodeFs from "node:fs";
import nodeAssert from 'node:assert';
import nodePath from 'node:path';

const comments = [
  '//! Credit to Astro | MIT License',
  // FIXME(hyf0): Following comments are not preserved https://github.com/oxc-project/oxc/issues/11093
  // '* @license Foo v1.0.0',
  // '* @license Bar v11.0.0',
  // '* MIT License',
]

const content = nodeFs.readFileSync(nodePath.join(import.meta.dirname, 'dist/main.js'), 'utf-8');

for (const comment of comments) {
  nodeAssert(content.includes(comment), `comment \`${comment}\` is not preserved`);
}
