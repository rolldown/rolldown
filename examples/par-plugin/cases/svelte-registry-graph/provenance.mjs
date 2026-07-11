import { createHash } from 'node:crypto';
import { readFile, readdir } from 'node:fs/promises';
import nodePath from 'node:path';

async function walk(directory) {
  const entries = await readdir(directory, { withFileTypes: true });
  const paths = await Promise.all(
    entries.map((entry) => {
      const path = nodePath.join(directory, entry.name);
      return entry.isDirectory() ? walk(path) : path;
    }),
  );
  return paths.flat();
}

export async function hashRolldownDistribution(repositoryRoot) {
  const distributionDirectory = nodePath.join(repositoryRoot, 'packages/rolldown/dist');
  const paths = (await walk(distributionDirectory)).sort((left, right) =>
    Buffer.compare(Buffer.from(left), Buffer.from(right)),
  );
  const aggregate = createHash('sha256');
  let bytes = 0;
  for (const path of paths) {
    const content = await readFile(path);
    const relativePath = nodePath
      .relative(distributionDirectory, path)
      .split(nodePath.sep)
      .join('/');
    const contentHash = createHash('sha256').update(content).digest('hex');
    bytes += content.byteLength;
    aggregate.update(relativePath);
    aggregate.update('\0');
    aggregate.update(String(content.byteLength));
    aggregate.update('\0');
    aggregate.update(contentHash);
    aggregate.update('\n');
  }
  return {
    directory: 'packages/rolldown/dist',
    files: paths.length,
    bytes,
    aggregateSha256: aggregate.digest('hex'),
  };
}
