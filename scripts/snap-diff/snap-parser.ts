import { snakeCase } from 'change-case';
import remarkParse from 'remark-parse';
import { unified } from 'unified';
export function parseEsbuildSnap(source: string) {
  let cases = source.split(
    '================================================================================',
  );
  return cases.map(parseEsbuildCase);
}

function parseEsbuildCase(source: string): {
  name: string;
  sourceList: { name: string; content: string }[];
} {
  let lines = source.trimStart().split('\n');
  let [name, ...rest] = lines;
  let trimmedName = name.slice(4);
  let normalizedName = snakeCase(trimmedName);
  let content = rest.join('\n');
  return { name: normalizedName, sourceList: parseContent(content) };
}

function parseContent(content: string) {
  // Define a regex pattern to match the filename and its content
  const regex = /----------\s*(.+?)\s*----------\s*([\s\S]*?)(?=----------|$)/g;

  const result = [];
  let match;

  // Use regex to find all matches in the input
  while ((match = regex.exec(content)) !== null) {
    const filename = match[1].trim(); // Extract the filename
    const content = match[2].trim(); // Extract the content

    // Push an object with filename and content into the result array
    result.push({
      name: filename,
      content: content,
    });
  }

  return result;
}

export function parseRolldownSnap(source: string | undefined) {
  if (!source) {
    return undefined;
  }
  let match;
  // strip `---source---` block
  while ((match = /---\n([\s\S]+?)\n---/.exec(source))) {
    source = source.slice(match.index + match[0].length);
  }
  // default mode

  const processor = unified().use(remarkParse);

  const parseTree = processor.parse(source);
  const tree: any = processor.runSync(parseTree);

  let i = 0;
  let inAsset = false;
  let ret = [];
  while (i < tree.children.length) {
    let child = tree.children[i];
    if (child.type === 'heading' && child.depth === 1) {
      let content = source.slice(
        child.position.start.offset,
        child.position.end.offset,
      );
      if (content.trim().slice(1).trim() === 'Assets') {
        inAsset = true;
      } else {
        inAsset = false;
      }
    }
    if (inAsset && child.type === 'heading' && child.depth === 2) {
      let content = source.slice(
        child.position.start.offset,
        child.position.end.offset,
      );
      let filename = content.trim().slice(2).trim();
      let codeBlock = tree.children[i + 1];
      if (codeBlock.type === 'code') {
        ret.push({
          filename,
          content: codeBlock.value,
        });
        i += 2;
        continue;
      }
    }
    i++;
  }
  return ret;
}
