import fg from 'fast-glob';
import * as fs from 'node:fs';
import * as path from 'node:path';
import remarkParse from 'remark-parse';
import { unified } from 'unified';

function extractReason(source: string) {
  const processor = unified().use(remarkParse);

  const parseTree = processor.parse(source);
  const tree: any = processor.runSync(parseTree);

  let i = 0;
  let inReason = false;
  let ret = [];

  while (i < tree.children.length) {
    let child = tree.children[i];
    if (inReason && child.type === 'list') {
      let childList = child.children;
      for (let j = 0; j < child.children.length; j++) {
        let listItem = childList[j];
        let position = listItem.children[0].position;
        let listContent = source.slice(
          position.start.offset,
          position.end.offset,
        );
        ret.push(listContent);
      }
    }
    if (child.type === 'heading' && child.depth === 1) {
      let content = source.slice(
        child.position.start.offset,
        child.position.end.offset,
      );
      if (content.trim().slice(1).trim() === 'Reason') {
        inReason = true;
      } else {
        inReason = false;
      }
    }
    i++;
  }
  return ret;
}

const workspaceDir = path.join(import.meta.dirname, '../..');

type AggregateReasonEntries = [string, string[]][];

export function aggregateReason(): AggregateReasonEntries {
  const entries = fg.globSync(['crates/rolldown/tests/esbuild/**/diff.md'], {
    dot: false,
    cwd: workspaceDir,
  });
  // a map for each directory to its diff reasons
  let reasonToCaseDirMap: Record<string, string[]> = {};
  for (let entry of entries) {
    const entryAbPath = path.resolve(workspaceDir, entry);
    let content = fs.readFileSync(entryAbPath, 'utf-8');
    let reasons = extractReason(content);
    let dirname = path.relative(workspaceDir, path.dirname(entryAbPath));
    const posixPath = dirname.replaceAll('\\', '/');

    for (let reason of reasons) {
      if (!reasonToCaseDirMap[reason]) {
        reasonToCaseDirMap[reason] = [];
      }
      reasonToCaseDirMap[reason].push(posixPath);
    }
  }
  let reverseMapEntries = Object.entries(reasonToCaseDirMap);
  for (let [_, dirs] of reverseMapEntries) {
    dirs.sort();
  }
  reverseMapEntries.sort((a, b) => {
    return b[1].length - a[1].length;
  });
  return reverseMapEntries;
}
