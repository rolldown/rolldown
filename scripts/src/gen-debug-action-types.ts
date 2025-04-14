import 'zx/globals';
import nodeFs from 'node:fs';
import nodePath from 'node:path';
import { REPO_ROOT } from '../meta/constants';

// https://github.com/Aleph-Alpha/ts-rs/blob/40b82771c8b63c986d6fcf8a71540d45816418c2/README.md?plain=1#L60-L61
// ts-rs uses `cargo test export_bindings` to trigger the generation of the type definition files.

await $`cargo test -p rolldown_debug_action export_bindings`;

const generatedTypesDir = nodePath.resolve(
  REPO_ROOT,
  'crates/rolldown_debug_action/bindings',
);

const generatedTypesFiles = nodeFs.readdirSync(generatedTypesDir);
generatedTypesFiles.sort();

if (generatedTypesFiles.length === 0) {
  throw new Error(`No generated types found in '${generatedTypesDir}'`);
}

const targetDir = nodePath.resolve(REPO_ROOT, 'packages/debug/src/generated');

// Clean up the target directory
if (nodeFs.existsSync(targetDir)) {
  nodeFs.rmSync(targetDir, { recursive: true, force: true });
}

// Copy the generated types to the target directory
nodeFs.mkdirSync(targetDir, { recursive: true });
for (const file of generatedTypesFiles) {
  const sourceFile = nodePath.resolve(generatedTypesDir, file);
  const targetFile = nodePath.resolve(targetDir, file);
  nodeFs.copyFileSync(sourceFile, targetFile);
}
// Clean up the generated types directory
nodeFs.rmSync(generatedTypesDir, { recursive: true, force: true });

// Create a `index.ts` file that re-exports all the generated types
const barrelFile = generatedTypesFiles
  .map((file) => `export * from './${file.slice(0, -3)}'`) // remove `.ts` extension
  .join('\n');

nodeFs.writeFileSync(
  nodePath.resolve(targetDir, 'index.ts'),
  barrelFile,
  'utf-8',
);

console.log('✅ Successfully generated devtool action types');
