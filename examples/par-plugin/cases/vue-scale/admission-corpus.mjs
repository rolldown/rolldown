import { createHash } from 'node:crypto';
import { readFile } from 'node:fs/promises';
import nodePath from 'node:path';
import { classifyVueSource, summarizeEntries } from './corpus.mjs';

export async function listQuasarPreExclusionEntries(corpusDirectory) {
  const support = JSON.parse(
    await readFile(nodePath.join(corpusDirectory, '.support-manifest.json'), 'utf8'),
  );
  const entries = [];
  for (const supportEntry of support.repositories?.quasar?.entries ?? []) {
    if (supportEntry.kind !== 'file' || !supportEntry.path.endsWith('.vue')) continue;
    const content = await readFile(nodePath.join(corpusDirectory, 'quasar', supportEntry.path));
    const classification = classifyVueSource(content, supportEntry.path);
    if (!classification.eligible) continue;
    entries.push({
      repository: 'quasar',
      path: supportEntry.path,
      sourceKey: `quasar/${supportEntry.path}`,
      bytes: content.byteLength,
      sha256: createHash('sha256').update(content).digest('hex'),
      kind: classification.kind,
    });
  }
  entries.sort((left, right) =>
    Buffer.compare(Buffer.from(left.sourceKey), Buffer.from(right.sourceKey)),
  );
  const seenContents = new Set();
  return entries.filter((entry) => {
    if (seenContents.has(entry.sha256)) return false;
    seenContents.add(entry.sha256);
    return true;
  });
}

export function summarizeAdmissionEntries(entries) {
  const aggregate = createHash('sha256');
  for (const entry of entries) {
    aggregate.update(entry.sourceKey);
    aggregate.update('\0');
    aggregate.update(String(entry.bytes));
    aggregate.update('\0');
    aggregate.update(entry.sha256);
    aggregate.update('\n');
  }
  return { ...summarizeEntries(entries), pathOrderedSha256: aggregate.digest('hex') };
}
