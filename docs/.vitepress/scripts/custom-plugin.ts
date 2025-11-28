import * as fs from 'node:fs';
import * as path from 'node:path';
import * as td from 'typedoc';

// Escape a string for use in a RegExp
function escapeRegex(s: string) {
  return s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}

// Try to extract the markdown section for a given property name from a
// parent page's rendered markdown `contents`.
function extractPropertySection(
  contents: string,
  propertyName: string,
): string | undefined {
  if (!contents) return undefined;
  const namePattern = escapeRegex(propertyName);

  // Match a heading that contains the property name (allow backticks around it).
  const headingRe = new RegExp(
    '^(#{1,6})\\s*(?:`?' + namePattern + '`?).*$',
    'm',
  );
  const m = contents.match(headingRe);
  if (!m) return undefined;

  const startIndex = m.index ?? 0;
  const startLevel = m[1].length;

  // Find next heading with level <= startLevel
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

  return contents.slice(startIndex, endIndex).trim();
}

export function load(app: td.Application) {
  // Track generated pages so we can update the sidebar after render.
  const generated: Record<string, Array<{ text: string; link: string }>> = {};

  app.renderer.on(
    td.Renderer.EVENT_END_PAGE,
    (page) => {
      // We only care about the specific pages you want to split
      if (
        page.model?.name === 'InputOptions' ||
        page.model?.name === 'OutputOptions'
      ) {
        const parentReflection = page.model as td.ContainerReflection;
        if (!parentReflection.children) return;

        const parentContents = String(page.contents ?? '');

        for (const property of parentReflection.children) {
          // Create a PageEvent for the single property reflection.
          const newPage = new td.PageEvent(property);

          // Set project, filename and url for the property's dedicated file.
          newPage.project = page.project;
          newPage.filename = `${parentReflection.name}.${property.name}.md`;
          newPage.url = `${parentReflection.name}.${property.name}.md`;

          // Try to extract the exact rendered markdown for this property from the
          // parent page contents. If that fails, fall back to a small generated
          // markdown file.
          const extracted = extractPropertySection(
            parentContents,
            property.name,
          );
          newPage.contents = extracted;

          // Write the generated markdown directly into the configured output
          // directory.
          const outDir = app.options?.getValue?.('out') ||
            './reference';
          const abs = path.resolve(outDir, newPage.url);
          fs.mkdirSync(path.dirname(abs), { recursive: true });
          fs.writeFileSync(abs, newPage.contents ?? '', 'utf8');
          // Record for later sidebar modification
          generated[parentReflection.name] ??= [];
          generated[parentReflection.name].push({
            text: property.name,
            link: `/${newPage.url.replace(/\\/g, '/')}`,
          });
        }

        // Prevent the original parent page from being written
        page.contents = '';
      }
    },
  );

  app.renderer.on(td.Renderer.EVENT_END, () => {
    const outDir = app.options?.getValue?.('out') || './reference';
    const optionsPath = path.resolve(outDir, 'options-sidebar.json');

    const sidebarArray = [];
    if (generated.InputOptions) {
      for (const item of generated.InputOptions) sidebarArray.push(item);
    }

    // For remaining groups, add them as grouped collapsed entries
    for (const parent of Object.keys(generated)) {
      if (parent === 'InputOptions') continue;
      const shortName = parent.replace(/Options$/, '') || parent;
      sidebarArray.push({
        text: shortName,
        collapsed: true,
        items: generated[parent],
      });
    }

    fs.writeFileSync(
      optionsPath,
      JSON.stringify(sidebarArray, null, 2),
      'utf8',
    );

    // Remove InputOptions and OutputOptions from typedoc-sidebar.json
    const typedocSidebarPath = path.resolve(outDir, 'typedoc-sidebar.json');
    if (fs.existsSync(typedocSidebarPath)) {
      const sidebar = JSON.parse(fs.readFileSync(typedocSidebarPath, 'utf8'));
      const filtered = filterSidebarEntries(sidebar, [
        'InputOptions',
        'OutputOptions',
      ]);
      fs.writeFileSync(
        typedocSidebarPath,
        JSON.stringify(filtered, null, 2),
        'utf8',
      );
    }
  });
}

// Recursively filter out sidebar entries by text name
function filterSidebarEntries(
  items: unknown[],
  namesToRemove: string[],
): unknown[] {
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
