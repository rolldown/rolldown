import * as fs from 'node:fs';
import * as path from 'node:path';
import * as td from 'typedoc';

function escapeRegex(s: string) {
  return s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}

const sourceFileCache = new Map<string, string>();
function readSourceFile(filePath: string): string {
  let contents = sourceFileCache.get(filePath);
  if (contents === undefined) {
    contents = fs.readFileSync(filePath, 'utf8');
    sourceFileCache.set(filePath, contents);
  }
  return contents;
}

/**
 * Walk upward from a property's source line to find the `/** ... *\/` JSDoc
 * block that immediately precedes it. Returns the raw text including the
 * opening and closing markers, or undefined if no adjacent block exists.
 */
function findJSDocAbove(sourceText: string, propertyLine: number): string | undefined {
  const lines = sourceText.split('\n');
  let end = propertyLine - 2;
  while (end >= 0 && lines[end].trim() === '') end--;
  if (end < 0 || !lines[end].trimEnd().endsWith('*/')) return undefined;
  let start = end;
  while (start >= 0 && !lines[start].trimStart().startsWith('/**')) start--;
  if (start < 0) return undefined;
  return lines.slice(start, end + 1).join('\n');
}

/**
 * Resolve the {@link include} targets of a property's JSDoc to absolute paths.
 * Mirrors TypeDoc's `{@include}` inline-tag semantics, but gives us a handle
 * on the original `.md` content before TypeDoc's block lexer rewrites escape
 * sequences — see https://github.com/rolldown/rolldown/issues/8792.
 */
function resolveIncludePaths(property: td.Reflection): string[] {
  const source = property.sources?.[0];
  if (!source) return [];
  const filePath = (source as { fullFileName?: string }).fullFileName ?? source.fileName;
  if (!filePath || !fs.existsSync(filePath)) return [];
  const jsdoc = findJSDocAbove(readSourceFile(filePath), source.line);
  if (!jsdoc) return [];
  const sourceDir = path.dirname(filePath);
  const targets: string[] = [];
  const includeRe = /\{@include\s+(\S+?\.md)\s*\}/g;
  let match: RegExpExecArray | null;
  while ((match = includeRe.exec(jsdoc)) !== null) {
    const resolved = path.resolve(sourceDir, match[1]);
    if (fs.existsSync(resolved)) targets.push(resolved);
  }
  return targets;
}

const fencedCodeRe = /^(```[^\n]*\n)([\s\S]*?)(^```)/gm;

/**
 * Locate the `[start, end)` slice of a single property's section inside a
 * rendered container page. Mirrors the heading-walk in
 * `extractPropertySection` but returns a range instead of mutated text, so
 * callers can splice a modified slice back into place.
 */
function findPropertyRange(
  contents: string,
  propertyName: string,
): { start: number; end: number } | undefined {
  const namePattern = escapeRegex(propertyName);
  const headingRe = new RegExp(
    '^(#{1,6})\\s*(?:~{2})?(?:`?' + namePattern + '\\b`?)(?:~{2})?.*$',
    'm',
  );
  const m = contents.match(headingRe);
  if (!m) return undefined;
  const start = m.index ?? 0;
  const startLevel = m[1].length;
  const nextHeadingRe = /^#{1,6}.*$/gm;
  nextHeadingRe.lastIndex = start + m[0].length;
  let end = contents.length;
  let nh: RegExpExecArray | null;
  while ((nh = nextHeadingRe.exec(contents)) !== null) {
    const hashes = nh[0].match(/^#{1,6}/);
    if (!hashes) continue;
    if (hashes[0].length <= startLevel) {
      end = nh.index;
      break;
    }
  }
  return { start, end };
}

/**
 * Swap the content of each fenced code block in `rendered` with the content
 * of the corresponding block (by position) from the concatenated raw
 * `{@include}` sources. TypeDoc's block lexer URL-encodes `[`, `\`, and `]`
 * inside injected code fences; restoring from the original `.md` sidesteps
 * that parser entirely without touching surrounding prose, heading
 * adjustments, or cross-reference links.
 *
 * No-ops when the block count differs, to avoid corrupting pages whose JSDoc
 * adds `@example` blocks on top of `{@include}` content.
 */
function restoreFencedCodeBlocks(rendered: string, includePaths: string[]): string {
  if (includePaths.length === 0) return rendered;
  const rawSource = includePaths.map(readSourceFile).join('\n\n');
  const rawBlocks = [...rawSource.matchAll(fencedCodeRe)].map((m) => m[2]);
  if (rawBlocks.length === 0) return rendered;
  const renderedBlockCount = [...rendered.matchAll(fencedCodeRe)].length;
  if (renderedBlockCount !== rawBlocks.length) return rendered;
  let i = 0;
  return rendered.replace(fencedCodeRe, (_, open, _body, close) => {
    return `${open}${rawBlocks[i++]}${close}`;
  });
}

/**
 * Parses a type reference from the option content's Type line only.
 * Matches patterns like: [`ChecksOptions`](Interface.ChecksOptions.md)
 * or [`TreeshakingOptions`](TypeAlias.TreeshakingOptions.md)
 */
function parseTypeReference(
  contents: string,
): { prefix: string; name: string; fullPath: string } | undefined {
  const typeLineMatch = contents.match(/^- \*\*Type\*\*: .+$/m);
  if (!typeLineMatch) return undefined;

  const typeLine = typeLineMatch[0];
  const match = typeLine.match(/\[`(\w+)`\]\((Interface|TypeAlias)\.\1\.md\)/);
  if (!match) return undefined;

  return {
    prefix: match[2],
    name: match[1],
    fullPath: `${match[2]}.${match[1]}.md`,
  };
}

/**
 * Extracts the Properties section from a Type or Interface markdown file.
 * Returns undefined if no Properties section exists.
 */
function extractPropertiesSection(contents: string): string | undefined {
  const match = contents.match(/^## Properties\n([\s\S]*)/m);
  if (!match) return undefined;

  let section = match[0];
  // Adjust heading levels: ### -> ##, #### -> ###
  section = section.replace(/^### /gm, '## ');
  section = section.replace(/^#### /gm, '### ');
  // Remove the "## Properties" heading itself since we'll integrate properties directly
  section = section.replace(/^## Properties\n+/m, '');
  // Remove horizontal separators between properties
  section = section.replace(/^\*\*\*$/gm, '');
  return section.trim();
}

function extractPropertySection(
  contents: string,
  propertyName: string,
  parentName: string,
  propertyNameMap: Map<string, string>,
): string | undefined {
  if (!contents) return undefined;
  const namePattern = escapeRegex(propertyName);
  const headingRe = new RegExp(
    '^(#{1,6})\\s*(?:~{2})?(?:`?' + namePattern + '\\b`?)(?:~{2})?.*$',
    'm',
  );
  const m = contents.match(headingRe);
  if (!m) return undefined;

  const startIndex = m.index ?? 0;
  const startLevel = m[1].length;

  const nextHeadingRe = /^#{1,6}.*$/gm;
  nextHeadingRe.lastIndex = startIndex + (m[0]?.length ?? 0);
  let endIndex = contents.length;
  let nh: RegExpExecArray | null;
  while ((nh = nextHeadingRe.exec(contents)) !== null) {
    const hashes = nh[0].match(/^#{1,6}/);
    if (!hashes) continue;
    const level = hashes[0].length;
    if (level <= startLevel) {
      endIndex = nh.index ?? endIndex;
      break;
    }
  }

  let section = contents.slice(startIndex, endIndex).trim();

  section =
    '<!-- This file is automatically generated by `docs/.vitepress/scripts/extract-options-plugin.ts`. DO NOT EDIT MANUALLY! -->\n' +
    section;

  return (
    section
      // Reduce each heading level by two (e.g., ### -> #)
      .replace(/^(#{3,6})/gm, (match) => match.slice(2))
      // Remove optional question mark from title (e.g. `# checks?` -> `# checks`)
      .replace(/^# (.+)\?\n?/m, '# $1')
      // Remove trailing horizontal separator
      .replace(/\*\*\*$/, '')
      // Transform in-page anchor links to cross-file links
      // e.g., [`output.file`](#file) -> [`output.file`](./OutputOptions.file)
      // Uses propertyNameMap to get correct casing (anchors are always lowercase)
      .replace(/\[([^\]]+)\]\(#([a-z][a-z0-9]*)\)/gi, (_, linkText, anchor) => {
        const propertyName = propertyNameMap.get(anchor.toLowerCase()) ?? anchor;
        return `[${linkText}](./${parentName}.${propertyName})`;
      })
  );
}

export function load(app: td.Application) {
  const generatedPage: Record<string, Array<{ text: string; link: string }>> = {};
  // Track which types were inlined and where they redirect to
  const inlinedTypes: Map<
    string,
    { typeFile: string; optionFile: string; optionName: string; parentName: string }
  > = new Map();

  // TypeDoc's block lexer URL-encodes `[`, `\`, `]` inside fenced code blocks
  // that are injected via `{@include}` tags. Run this first on every
  // container page so the subsequent per-property extractor reads clean
  // content, and so interface pages like Interface.WatchOptions are fixed
  // in place too. See https://github.com/rolldown/rolldown/issues/8792.
  app.renderer.on(td.Renderer.EVENT_END_PAGE, (page) => {
    const model = page.model;
    if (!(model instanceof td.ContainerReflection) || !model.children) return;
    let contents = page.contents ?? '';
    let changed = false;
    for (const property of model.children) {
      const includes = resolveIncludePaths(property);
      if (includes.length === 0) continue;
      const range = findPropertyRange(contents, property.name);
      if (!range) continue;
      const slice = contents.slice(range.start, range.end);
      const restored = restoreFencedCodeBlocks(slice, includes);
      if (restored !== slice) {
        contents = contents.slice(0, range.start) + restored + contents.slice(range.end);
        changed = true;
      }
    }
    if (changed) page.contents = contents;
  });

  app.renderer.on(td.Renderer.EVENT_END_PAGE, (page) => {
    if (page.model?.name === 'InputOptions' || page.model?.name === 'OutputOptions') {
      const parentReflection = page.model as td.ContainerReflection;
      if (!parentReflection.children) return;

      const parentContents = page.contents ?? '';

      const propertyNameMap = new Map<string, string>();
      for (const prop of parentReflection.children) {
        propertyNameMap.set(prop.name.toLowerCase(), prop.name);
      }

      for (const property of parentReflection.children) {
        const newPage = new td.PageEvent(property);

        newPage.project = page.project;
        newPage.filename = `${parentReflection.name}.${property.name}.md`;
        newPage.url = `${parentReflection.name}.${property.name}.md`;

        const extracted = extractPropertySection(
          parentContents,
          property.name,
          parentReflection.name,
          propertyNameMap,
        );
        newPage.contents = extracted;

        // Check if this option references a TypeAlias or Interface with properties
        if (extracted) {
          const typeRef = parseTypeReference(extracted);
          if (typeRef) {
            if (inlinedTypes.has(typeRef.name)) {
              throw new Error(
                `Type reference "${typeRef.name}" is referenced by option "${parentReflection.name}.${property.name}" but it is also referenced by another option.`,
              );
            }

            // Track the type for later inlining in EVENT_END
            // (the type file might not exist yet if processed before the type page)
            inlinedTypes.set(typeRef.name, {
              typeFile: typeRef.fullPath,
              optionFile: newPage.url,
              optionName: property.name,
              parentName: parentReflection.name,
            });
          }
        }

        const outDir = app.options.getValue('out');
        const abs = path.resolve(outDir, newPage.url);
        fs.mkdirSync(path.dirname(abs), { recursive: true });
        fs.writeFileSync(abs, newPage.contents ?? '', 'utf8');

        // Record for later sidebar modification
        generatedPage[parentReflection.name] ??= [];
        generatedPage[parentReflection.name].push({
          text: property.name,
          link: `/${newPage.url.replaceAll('\\', '/')}`,
        });
      }
    }
  });

  app.renderer.on(td.Renderer.EVENT_END, () => {
    const outDir = app.options.getValue('out');

    // Inline types
    const inlinedTypeNames: string[] = [];
    for (const [typeName, info] of inlinedTypes) {
      const typeFilePath = path.resolve(outDir, info.typeFile);
      if (!fs.existsSync(typeFilePath)) continue;

      const typeContents = fs.readFileSync(typeFilePath, 'utf8');
      const propertiesSection = extractPropertiesSection(typeContents);
      if (!propertiesSection) continue;

      const optionFilePath = path.resolve(outDir, info.optionFile);
      let optionContents = fs.readFileSync(optionFilePath, 'utf8');

      // Replace the type reference with "object with the properties below"
      // Preserves any prefix like `boolean` \| before the type reference
      // Matches patterns like: - **Type**: [`TypeName`](Interface.TypeName.md)
      // or: - **Type**: `boolean` \| [`TypeName`](TypeAlias.TypeName.md)
      // Use the specific type name to avoid matching nested type references
      const escapedTypeName = escapeRegex(typeName);
      optionContents = optionContents.replace(
        new RegExp(
          `^(- \\*\\*Type\\*\\*: )(.*?)\\[\`${escapedTypeName}\`\\]\\((Interface|TypeAlias)\\.${escapedTypeName}\\.md\\)(.*)$`,
          'm',
        ),
        (_, prefix, before, _typePrefix, after) => {
          return `${prefix}${before}object with the properties below${after}`;
        },
      );

      const updatedContents = optionContents.trimEnd() + '\n\n' + propertiesSection + '\n';
      fs.writeFileSync(optionFilePath, updatedContents, 'utf8');

      // Determine if it's an Interface or TypeAlias for the redirect heading
      const isInterface = info.typeFile.startsWith('Interface.');
      const typeLabel = isInterface ? 'Interface' : 'Type Alias';

      // Create redirect file for the type
      const redirectContents = `# ${typeLabel}: ${typeName}

See [${info.parentName}.${info.optionName}](${info.optionFile})
`;
      fs.writeFileSync(typeFilePath, redirectContents, 'utf8');

      inlinedTypeNames.push(typeName);
    }

    const optionsPath = path.resolve(outDir, 'options-sidebar.json');

    const sidebarArray = [];
    if (generatedPage.InputOptions) {
      for (const item of generatedPage.InputOptions) sidebarArray.push(item);
    }

    // Add Output options as grouped collapsed entries
    for (const parent of Object.keys(generatedPage)) {
      if (parent === 'InputOptions') continue;
      sidebarArray.push({
        text: 'output',
        collapsed: true,
        items: generatedPage[parent],
      });
    }

    fs.writeFileSync(optionsPath, JSON.stringify(sidebarArray, null, 2), 'utf8');

    // Remove InputOptions, OutputOptions, and inlined types from typedoc-sidebar.json
    const typedocSidebarPath = path.resolve(outDir, 'typedoc-sidebar.json');
    if (fs.existsSync(typedocSidebarPath)) {
      const sidebar = JSON.parse(fs.readFileSync(typedocSidebarPath, 'utf8'));
      const namesToRemove = ['InputOptions', 'OutputOptions', ...inlinedTypeNames];
      const filtered = filterSidebarEntries(sidebar, namesToRemove);
      fs.writeFileSync(typedocSidebarPath, JSON.stringify(filtered, null, 2), 'utf8');
    }
  });
}

function filterSidebarEntries(items: unknown[], namesToRemove: string[]): unknown[] {
  return items
    .filter((item: any) => !namesToRemove.includes(item.text))
    .map((item: any) => {
      if (item.items) {
        return {
          ...item,
          items: filterSidebarEntries(item.items, namesToRemove),
        };
      }
      return item;
    });
}
