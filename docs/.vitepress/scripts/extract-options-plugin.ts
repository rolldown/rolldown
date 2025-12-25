import * as fs from 'node:fs';
import * as path from 'node:path';
import * as td from 'typedoc';

function escapeRegex(s: string) {
  return s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}

function extractPropertySection(
  contents: string,
  propertyName: string,
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

  const section = contents.slice(startIndex, endIndex).trim();

  // Reduce each heading level by two (e.g., ### -> #)
  return section.replace(/^(#{2,6})/gm, (match) => match.slice(2));
}

export function load(app: td.Application) {
  const generatedPage: Record<string, Array<{ text: string; link: string }>> =
    {};

  app.renderer.on(
    td.Renderer.EVENT_END_PAGE,
    (page) => {
      if (
        page.model?.name === 'InputOptions' ||
        page.model?.name === 'OutputOptions'
      ) {
        const parentReflection = page.model as td.ContainerReflection;
        if (!parentReflection.children) return;

        const parentContents = String(page.contents ?? '');

        for (const property of parentReflection.children) {
          const newPage = new td.PageEvent(property);

          newPage.project = page.project;
          newPage.filename = `${parentReflection.name}.${property.name}.md`;
          newPage.url = `${parentReflection.name}.${property.name}.md`;

          const extracted = extractPropertySection(
            parentContents,
            property.name,
          );
          newPage.contents = extracted;

          const outDir = app.options?.getValue?.('out');
          const abs = path.resolve(outDir, newPage.url);
          fs.mkdirSync(path.dirname(abs), { recursive: true });
          fs.writeFileSync(abs, newPage.contents ?? '', 'utf8');

          // Record for later sidebar modification
          generatedPage[parentReflection.name] ??= [];
          generatedPage[parentReflection.name].push({
            text: property.name,
            link: `/${newPage.url.replace(/\\/g, '/')}`,
          });
        }
      }
    },
  );

  app.renderer.on(td.Renderer.EVENT_END, () => {
    const outDir = app.options?.getValue?.('out');
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
