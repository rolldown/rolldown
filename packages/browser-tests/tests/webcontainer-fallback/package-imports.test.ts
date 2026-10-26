// WebContainer runs the packed packages on Node, which refuses to load a declaration file from
// node_modules (ERR_UNSUPPORTED_NODE_MODULES_TYPE_STRIPPING). A shared chunk that imports one
// still bundles and type-checks, so only the packed files show it.
import { execFileSync } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { expect, test } from 'vitest';

const fixtures = path.resolve('tests/fixtures');

// rolldown prints each static import at the start of a line in double quotes, which keeps
// JSDoc `import('./types')` and the dev runtime inlined as a string out of the match
const RELATIVE_IMPORT =
  /^(?:import|export)\b[^;]*?\bfrom\s*"(\.{1,2}\/[^"]+)"|^import\s*"(\.{1,2}\/[^"]+)"|\b(?:import|require)\("(\.{1,2}\/[^"]+)"\)/gm;

test.each(['browser/rolldown-browser.tgz', 'node/rolldown.tgz'])(
  '%s only imports JavaScript it ships',
  (tarball) => {
    const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'rolldown-package-imports-'));
    try {
      execFileSync('tar', ['-xzf', path.join(fixtures, tarball), '-C', dir]);
      const root = path.join(dir, 'package');
      const imports: string[] = [];
      const problems: string[] = [];
      for (const file of fs.readdirSync(root, { recursive: true, encoding: 'utf8' })) {
        if (!/\.[cm]?js$/.test(file)) continue;
        const code = fs.readFileSync(path.join(root, file), 'utf8');
        for (const match of code.matchAll(RELATIVE_IMPORT)) {
          const specifier = match[1] ?? match[2] ?? match[3];
          imports.push(specifier);
          if (/\.d\.[cm]?ts$/.test(specifier)) {
            problems.push(`${file} imports the declaration file ${specifier}`);
          } else if (!fs.existsSync(path.resolve(root, path.dirname(file), specifier))) {
            problems.push(`${file} imports ${specifier}, which the package does not ship`);
          }
        }
      }
      // a regex that stopped matching rolldown's output would otherwise pass vacuously
      expect(imports.length).toBeGreaterThan(0);
      expect(problems).toEqual([]);
    } finally {
      fs.rmSync(dir, { recursive: true, force: true });
    }
  },
);
