
import nodeFs from "node:fs";
import nodeAssert from 'node:assert';
import nodePath from 'node:path';

const comments = [
  '/** @license foo1 */',
  '/** foo2 */',
  '/** bar */',
  '/** bar2 */',
]

const content = nodeFs.readFileSync(nodePath.join(import.meta.dirname, 'dist/main.js'), 'utf-8');

for (const comment of comments) {
  nodeAssert(!content.includes(comment), `comment ${comment} not be preserved`);
}
