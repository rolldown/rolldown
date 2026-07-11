import { writeFile } from 'node:fs/promises';
import nodePath from 'node:path';

const exactPadding = (byteLength) => {
  if (byteLength <= 0) return '';
  if (byteLength < 4) return ' '.repeat(byteLength);
  return `/*${'s'.repeat(byteLength - 4)}*/`;
};

export async function generateControlledCorpus({
  corpusDirectory,
  graphShape,
  moduleCount,
  minimumSourceBytes,
}) {
  const padSource = (source) =>
    `${source}${exactPadding(Math.max(0, minimumSourceBytes - Buffer.byteLength(source)))}`;
  let totalSourceBytes = 0;

  for (let index = 0; index < moduleCount; index++) {
    const nextImport =
      graphShape === 'chain' && index + 1 < moduleCount
        ? `import './module-${index + 1}.controlled.js';\n`
        : '';
    const source = padSource(
      `${nextImport}globalThis.__controlled = (globalThis.__controlled || 0) + ${index};\n`,
    );
    totalSourceBytes += Buffer.byteLength(source);
    await writeFile(nodePath.join(corpusDirectory, `module-${index}.controlled.js`), source);
  }

  const entrySource = padSource(
    graphShape === 'wide'
      ? `${Array.from(
          { length: moduleCount },
          (_, index) => `import './module-${index}.controlled.js';`,
        ).join('\n')}\n`
      : "import './module-0.controlled.js';\n",
  );
  totalSourceBytes += Buffer.byteLength(entrySource);
  await writeFile(nodePath.join(corpusDirectory, 'entry.controlled.js'), entrySource);
  return totalSourceBytes;
}
