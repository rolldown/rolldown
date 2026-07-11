import { writeFile } from 'node:fs/promises';
import nodePath from 'node:path';

export async function generateControlledHookCorpus({
  corpusDirectory,
  hook,
  graphShape,
  moduleCount,
}) {
  const prefix = hook === 'resolveId' ? 'controlled-resolve:' : 'controlled-load:';
  const entrySource =
    graphShape === 'wide'
      ? `${Array.from({ length: moduleCount }, (_, index) => `import '${prefix}${index}';`).join(
          '\n',
        )}\n`
      : `import '${prefix}0';\n`;
  await writeFile(nodePath.join(corpusDirectory, 'entry.js'), entrySource);
  await writeFile(nodePath.join(corpusDirectory, 'fs-probe.txt'), 'controlled hook fs probe\n');
  return { entrySourceBytes: Buffer.byteLength(entrySource) };
}
