// Generate eight entry chunks and eight shared chunks for hook-path benchmarks.
// Usage: node chunk-hook-workload.mjs --out /tmp/chunk-hook --modules 12 --functions 60
// The destination must be empty. Build workload.json's entries together with
// format: 'esm', minify: true (or 'dce-only'), and sourcemap: true.
import { mkdir, readdir, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { parseArgs } from 'node:util';

const { values } = parseArgs({
  options: {
    out: { type: 'string' },
    modules: { type: 'string', default: '12' },
    functions: { type: 'string', default: '60' },
  },
});
if (!values.out) throw new Error('--out is required');
const modules = Number(values.modules);
const functions = Number(values.functions);
for (const value of [modules, functions]) {
  if (!Number.isSafeInteger(value) || value < 1) throw new Error('Sizes must be positive integers');
}
const root = path.resolve(values.out);
await mkdir(root, { recursive: true });
if ((await readdir(root)).length) throw new Error('Destination must be empty');

async function group(directory, seed) {
  await mkdir(path.join(root, directory), { recursive: true });
  const imports = [];
  const names = [];
  for (let module = 0; module < modules; module++) {
    const declarations = [];
    const exports = [];
    for (let index = 0; index < functions; index++) {
      const name = `operation${index}`;
      const salt = seed * 100003 + module * 101 + index + 1;
      declarations.push(`function ${name}(value, flag) {
  const bias = ${salt % 997};
  let scaled = value * ${(salt % 31) + 1} + bias;
  if (flag && scaled > ${salt % 701}) {
    scaled = (scaled ^ ${salt}) + value / ${(salt % 17) + 1};
  } else {
    scaled = Math.imul(scaled | 0, ${(salt % 127) + 1}) - bias;
  }
  const label = "group-${seed}-module-${module}-operation-${index}";
  return { label, value: scaled, valid: Number.isFinite(scaled), original: value };
}`);
      exports.push(name);
    }
    await writeFile(
      path.join(root, directory, `module-${module}.js`),
      `${declarations.join('\n')}\nexport const operations = [${exports.join(', ')}];\n`,
    );
    imports.push(`import { operations as part${module} } from './module-${module}.js';`);
    names.push(`...part${module}`);
  }
  await writeFile(
    path.join(root, directory, 'index.js'),
    `${imports.join('\n')}\nexport const operations = [${names.join(', ')}];\n`,
  );
}

const entries = {};
for (let entry = 0; entry < 8; entry++) {
  await group(`shared-${entry}`, entry + 1);
  await group(`private-${entry}`, entry + 101);
  const filename = `entry-${entry}.js`;
  entries[`entry-${entry}`] = filename;
  // Each shared group is consumed by exactly two adjacent entries. The eight
  // distinct consumer sets form eight shared chunks without manual chunk rules.
  await writeFile(
    path.join(root, filename),
    `import { operations as own } from './private-${entry}/index.js';
import { operations as left } from './shared-${entry}/index.js';
import { operations as right } from './shared-${(entry + 7) % 8}/index.js';
export const operations = [...own, ...left, ...right];
export function run(value, selector) {
  return operations[(selector >>> 0) % operations.length](value, !!(selector & 1));
}
`,
  );
}
await writeFile(path.join(root, 'package.json'), '{"type":"module","private":true}\n');
const manifest = {
  entries,
  expectedChunks: 16,
  modulesPerGroup: modules,
  functionsPerModule: functions,
};
await writeFile(path.join(root, 'workload.json'), `${JSON.stringify(manifest, null, 2)}\n`);
console.log(JSON.stringify({ directory: root, ...manifest }));
