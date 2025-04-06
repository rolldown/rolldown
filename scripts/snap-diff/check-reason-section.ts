import fg from 'fast-glob';
import * as fs from 'node:fs';
import * as path from 'node:path';
import { extractReason, workspaceDir } from './aggregate-reason';

function ensureReasonSection() {
  const entries = fg.globSync(['crates/rolldown/tests/esbuild/**/diff.md'], {
    dot: false,
    cwd: workspaceDir,
  });
  for (let entry of entries) {
    // skip `lower` since they both have same reason
    if (entry.startsWith('crates/rolldown/tests/esbuild/lower')) {
      continue;
    }
    const entryAbPath = path.resolve(workspaceDir, entry);
    let content = fs.readFileSync(entryAbPath, 'utf-8');
    let reasons = extractReason(content);
    if (reasons.length === 0) {
      console.log(`entry: `, entry);
    }
  }
}

ensureReasonSection();
