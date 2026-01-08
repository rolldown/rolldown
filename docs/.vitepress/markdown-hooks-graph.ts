import { Graphviz } from '@hpcc-js/wasm-graphviz';
import type MarkdownIt from 'markdown-it';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';

const RENDERER_VERSION = 1;

type StyleDef = {
  light: Record<string, string>;
  dark: Record<string, string>;
};
type StyleMap = Record<string, StyleDef>;
type NodeDef = { name: string; styles: string[]; href?: string };
type GroupMap = Record<string, string[]>;
type EdgeDef = {
  from: string;
  to: string;
  label?: string;
  dotted: boolean;
  constraint?: boolean;
};

type ParsedDsl = {
  styles: StyleMap;
  nodes: NodeDef[];
  groups: GroupMap;
  edges: EdgeDef[];
  excludeFromLegend: Set<string>;
  legendPosition: 'L' | 'R';
  maxWidth: string;
  marginX: string;
  marginY: string;
};

const SECTION_CONFIG = 'config';
const SECTION_STYLES = 'styles';
const SECTION_NODES = 'nodes';
const SECTION_GROUPS = 'groups';
const SECTION_EDGES = 'edges';

type Section =
  | typeof SECTION_CONFIG
  | typeof SECTION_STYLES
  | typeof SECTION_NODES
  | typeof SECTION_GROUPS
  | typeof SECTION_EDGES
  | null;

function getThemeColor(mode: 'light' | 'dark'): Record<string, string> {
  return { text: /* --vp-c-text-1 */ mode === 'dark' ? '#dfdfd6' : '#3c3c43' };
}

function ensureQuoted(value: string): string {
  const trimmed = value.trim();
  const isNumber = /^-?\d+(?:\.\d+)?$/.test(trimmed);
  const alreadyQuoted = /^".*"$/.test(trimmed);
  return isNumber || alreadyQuoted ? trimmed : `"${trimmed}"`;
}

function parseAttributeList(raw: string): StyleDef {
  const light: Record<string, string> = {};
  const dark: Record<string, string> = {};
  const tokens = raw
    .split(',')
    .map((token) => token.trim())
    .filter(Boolean);

  for (const token of tokens) {
    const hasKey = token.includes('=');
    if (!hasKey) {
      console.warn(`Invalid style attribute: ${token}`);
      continue;
    }
    const [rawKey, rawValue] = token.split(/=/, 2);
    const key = rawKey.trim();
    const value = ensureQuoted(rawValue.trim());

    if (key.startsWith('dark$')) {
      const actualKey = key.substring(5);
      dark[actualKey] = value;
    } else {
      light[key] = value;
      // Copy to dark if no dark-specific value
      if (!key.includes('dark$')) {
        dark[key] = value;
      }
    }
  }

  return { light, dark };
}

function parseDsl(text: string): ParsedDsl {
  const styles: StyleMap = {};
  const nodes: NodeDef[] = [];
  const groups: GroupMap = {};
  const edges: EdgeDef[] = [];
  const excludeFromLegend = new Set<string>();

  let section: Section = null;
  let legendPosition: 'L' | 'R' = 'R';
  let maxWidth = '';
  let marginX = '';
  let marginY = '';

  for (const rawLine of text.split(/\r?\n/)) {
    const line = rawLine.trim();
    if (!line) continue;

    if (line.startsWith('#')) {
      const header = line.replace(/^#\s*/, '').toLowerCase();
      if (
        [SECTION_CONFIG, SECTION_STYLES, SECTION_NODES, SECTION_GROUPS, SECTION_EDGES].includes(
          header,
        )
      ) {
        section = header as Section;
      }
      continue;
    }

    if (!section) continue;

    if (section === SECTION_CONFIG) {
      const [key, value] = line.split(/=/, 2).map((chunk) => chunk.trim());
      if (key === 'legendPosition' && (value === 'L' || value === 'R')) {
        legendPosition = value;
      } else if (key === 'maxWidth') {
        maxWidth = value;
      } else if (key === 'margin') {
        const parts = value.split(',').map((p) => p.trim());
        marginX = parts[0] || '';
        marginY = parts[1] || '';
      }
      continue;
    }

    if (section === SECTION_STYLES) {
      const [name, definition] = line.split(/:/, 2).map((chunk) => chunk.trim());
      if (!name || !definition) continue;
      const styleName = name.toLowerCase();
      const actualStyleName = styleName.startsWith('!') ? styleName.slice(1) : styleName;
      if (styleName.startsWith('!')) {
        excludeFromLegend.add(actualStyleName);
      }
      styles[actualStyleName] = parseAttributeList(definition);
      continue;
    }

    if (section === SECTION_NODES) {
      // Find the closing parenthesis to handle URLs with colons
      const parenIndex = line.indexOf(')');
      let nodeDefPart: string;
      let styleList: string | undefined;

      if (parenIndex !== -1) {
        // Has parentheses, find colon after closing paren
        const afterParen = line.substring(parenIndex + 1);
        const colonIndex = afterParen.indexOf(':');
        if (colonIndex !== -1) {
          nodeDefPart = line.substring(0, parenIndex + 1 + colonIndex).trim();
          styleList = afterParen.substring(colonIndex + 1).trim();
        } else {
          nodeDefPart = line.trim();
        }
      } else {
        // No parentheses, split normally
        const [part1, part2] = line.split(/:/, 2).map((chunk) => chunk.trim());
        nodeDefPart = part1;
        styleList = part2;
      }

      if (!nodeDefPart) continue;

      // Parse nodeName(url) syntax
      let nodeName = nodeDefPart;
      let href: string | undefined;
      const urlMatch = nodeDefPart.match(/^(.+?)\((.+?)\)$/);
      if (urlMatch) {
        nodeName = urlMatch[1].trim();
        href = urlMatch[2].trim();
      }

      const styleNames = styleList
        ? styleList
            .split(',')
            .map((item) => item.trim().toLowerCase())
            .filter(Boolean)
        : [];
      nodes.push({ name: nodeName, styles: styleNames, href });
      continue;
    }

    if (section === SECTION_GROUPS) {
      const [groupName, members] = line.split(/:/, 2).map((chunk) => chunk.trim());
      if (!groupName || !members) continue;
      groups[groupName] = members
        .split(',')
        .map((member) => member.trim())
        .filter(Boolean);
      continue;
    }

    if (section === SECTION_EDGES) {
      const [edgePart, maybeLabel] = line.split(/:/, 2).map((chunk) => chunk.trim());
      if (!edgePart) continue;
      const match = edgePart.match(/^(.*?)\s*(\.->|->)\s*(.*?)$/);
      if (!match) continue;
      const [, from, connector, to] = match;

      let label: string | undefined;
      let constraint: boolean | undefined;

      if (maybeLabel) {
        const parts = maybeLabel.split(',').map((p) => p.trim());
        for (const part of parts) {
          if (part.startsWith('constraint=')) {
            const value = part.substring('constraint='.length);
            constraint = value.toLowerCase() === 'true';
          } else {
            label = part;
          }
        }
      }

      edges.push({
        from: from.trim(),
        to: to.trim(),
        label,
        dotted: connector === '.->',
        constraint,
      });
    }
  }

  return {
    styles,
    nodes,
    groups,
    edges,
    excludeFromLegend,
    legendPosition,
    maxWidth,
    marginX,
    marginY,
  };
}

function slugify(name: string): string {
  return name.replace(/[^a-zA-Z0-9]/g, '').toLowerCase();
}

function formatAttributes(attrs: Record<string, string>): string {
  const entries: Array<[string, string]> = [];
  if (attrs.label) entries.push(['label', attrs.label]);
  for (const [key, value] of Object.entries(attrs)) {
    if (key === 'label') continue;
    entries.push([key, value]);
  }
  return entries.map(([key, value]) => `${key}=${value}`).join(', ');
}

function removeQuotes(value: string | undefined): string | undefined {
  return value?.replace(/^"(.*)"$/, '$1');
}

function buildLegendTable(parsed: ParsedDsl, mode: 'light' | 'dark'): string | null {
  const textColor = getThemeColor(mode).text;
  const legendStyles = Object.keys(parsed.styles).filter(
    (styleName) => !parsed.excludeFromLegend.has(styleName),
  );
  const legendRows: string[] = [];

  for (const styleName of legendStyles) {
    const styleDef = parsed.styles[styleName]!;
    const attrs = styleDef[mode];

    const fillColor = removeQuotes(attrs.fillcolor) ?? 'transparent';
    const borderColor = removeQuotes(attrs.color) ?? 'transparent';
    const labelColor = removeQuotes(attrs.fontcolor) ?? textColor;

    legendRows.push(
      `<tr style="background:transparent"><td style="width:8px;height:8px;border:2px solid ${borderColor};background:${fillColor}"></td>` +
        `<td style="padding:2px 4px;color:${labelColor};white-space:nowrap;text-align:left;border:none">${styleName}</td></tr>`,
    );
  }

  if (!legendRows.length) return null;

  return (
    `<table style="margin:0; border:1px solid ${textColor};border-radius:6px;border-collapse:separate;border-spacing:4px;padding:6px;background:transparent">` +
    '<tbody>' +
    `<tr style="background:transparent"><td colspan="2" style="border:0;padding:0;text-align:right;font-weight:bold;color:${textColor};">Legend</td></tr>` +
    legendRows.join('') +
    '</tbody>' +
    '</table>'
  );
}

function appendLegend(svg: string, legendHtml: string, position: 'L' | 'R' = 'R'): string {
  const justifyContent = position === 'L' ? 'flex-start' : 'flex-end';
  const wrapper =
    `<g class="hooks-legend">` +
    `<foreignObject x="0" y="0" width="100%" height="100%" style="pointer-events:none">` +
    `<div xmlns="http://www.w3.org/1999/xhtml" style="width:100%;height:100%;display:flex;justify-content:${justifyContent};align-items:flex-start;box-sizing:border-box;padding:6px;">` +
    `<div style="pointer-events:auto">${legendHtml}</div>` +
    `</div>` +
    `</foreignObject>` +
    `</g>`;
  return svg.replace(/<\/svg>/, `${wrapper}</svg>`);
}

function buildNodeAttributes(
  node: NodeDef,
  styles: StyleMap,
  mode: 'light' | 'dark',
): Record<string, string> {
  const attrs: Record<string, string> = { label: ensureQuoted(node.name) };
  for (const styleName of node.styles) {
    const styleDef = styles[styleName];
    if (!styleDef) {
      throw new Error(`Unknown style: ${styleName} on node ${node.name}`);
    }
    Object.assign(attrs, styleDef[mode]);
  }

  if (removeQuotes(attrs.shape) !== 'circle') {
    const existingStyle = attrs.style ?? '';
    attrs.style = existingStyle.includes('rounded')
      ? existingStyle
      : existingStyle
        ? `"${existingStyle.replace(/^"(.*)"$/, '$1')},filled,rounded"`
        : '"filled,rounded"';
  } else {
    const borderColor = attrs.color ?? getThemeColor(mode).text;
    attrs.fillcolor = `"${borderColor}"`;
  }
  attrs.color ??= 'transparent';

  if (node.href) {
    attrs.href = ensureQuoted(node.href);
  }

  return attrs;
}

function toDot(parsed: ParsedDsl, mode: 'light' | 'dark' = 'light'): string {
  const nodeIdBySlug = new Map<string, string>();
  const nodeAttrsById = new Map<string, Record<string, string>>();

  for (const node of parsed.nodes) {
    const id = slugify(node.name);
    nodeIdBySlug.set(id, node.name);
    nodeAttrsById.set(id, buildNodeAttributes(node, parsed.styles, mode));
  }

  const groupByNode = new Map<string, string>();
  for (const [groupName, members] of Object.entries(parsed.groups)) {
    for (const member of members) {
      groupByNode.set(slugify(member), groupName);
    }
  }

  const edges = parsed.edges.map((edge) => {
    const fromId = slugify(edge.from);
    const toId = slugify(edge.to);
    if (!nodeAttrsById.has(fromId)) {
      throw new Error(`Edge references unknown from-node: ${edge.from}`);
    }
    if (!nodeAttrsById.has(toId)) {
      throw new Error(`Edge references unknown to-node: ${edge.to}`);
    }
    return { ...edge, from: fromId, to: toId };
  });

  const clusterEdges = edges.filter((edge) => {
    const fromGroup = groupByNode.get(edge.from);
    const toGroup = groupByNode.get(edge.to);
    return fromGroup && toGroup && fromGroup === toGroup;
  });
  const mainEdges = edges.filter((edge) => !clusterEdges.includes(edge));

  const lines: string[] = [];
  lines.push('digraph {');
  lines.push('    bgcolor="transparent";');
  lines.push('    rankdir=TB;');

  const textColor = getThemeColor(mode).text;
  lines.push(
    `    node [shape=box, style=filled, fontname="Arial", margin="0.2,0.1", color="${textColor}", fontcolor="${textColor}"];`,
  );
  lines.push(`    edge [fontname="Arial", color="${textColor}"];`);
  lines.push('');
  lines.push('    // Node definitions with styling');

  for (const [id, attrs] of nodeAttrsById.entries()) {
    const attrText = formatAttributes(attrs);
    lines.push(`    ${id} [${attrText}];`);
  }

  const clusterNames = Array.from(
    new Set(clusterEdges.map((edge) => groupByNode.get(edge.from) as string)),
  );

  const formatEdge = (edge: EdgeDef, indent: string) => {
    const attrs: Record<string, string> = {};
    if (edge.label) {
      attrs.label = ensureQuoted(edge.label);
      attrs.fontcolor = `"${textColor}"`;
    }
    if (edge.dotted) {
      attrs.style = 'dashed';
    }
    if (edge.constraint === false) {
      attrs.constraint = 'false';
    }
    attrs.penwidth = '2';
    const attrText = formatAttributes(attrs);
    const suffix = attrText ? ` [${attrText}]` : '';
    lines.push(`${indent}${edge.from} -> ${edge.to}${suffix};`);
  };

  if (mainEdges.length) {
    lines.push('');
    lines.push('    // Main flow');
    for (const edge of mainEdges) {
      formatEdge(edge, '    ');
    }
  }

  if (clusterNames.length) {
    lines.push('');
    lines.push('    // Subgraphs');
  }

  for (const groupName of clusterNames) {
    lines.push(`    subgraph cluster_${slugify(groupName)} {`);
    lines.push('        style=invis;');
    lines.push('        label="";');
    lines.push('');
    const edgesInGroup = clusterEdges.filter((edge) => groupByNode.get(edge.from) === groupName);
    for (const edge of edgesInGroup) {
      formatEdge(edge, '        ');
    }
    lines.push('    }');
  }

  lines.push('}');

  return lines.join('\n');
}

let graphvizInstance: Awaited<ReturnType<typeof Graphviz.load>> | null = null;

function dslToDot(dsl: string): string {
  const parsed = parseDsl(dsl);
  return toDot(parsed);
}

export async function hooksGraphPlugin(md: MarkdownIt): Promise<void> {
  // Load Graphviz instance once during plugin initialization
  graphvizInstance = await Graphviz.load();

  const defaultFenceRenderer = md.renderer.rules.fence!;

  md.renderer.rules.fence = (tokens, idx, options, env, self) => {
    const token = tokens[idx];
    const info = token.info.trim();

    // hooks-graph:txt → convert to DOT and output as code block
    if (info === 'hooks-graph:txt') {
      const dsl = token.content;
      const dot = dslToDot(dsl);
      token.content = dot;
      token.info = 'txt:line-numbers';
      return defaultFenceRenderer([token], 0, options, env, self);
    }

    // hooks-graph → convert to SVG and output as image
    if (info === 'hooks-graph') {
      const dsl = token.content;
      const parsed = parseDsl(dsl);
      const hash = crypto
        .createHash('sha256')
        .update(`${RENDERER_VERSION}:${graphvizInstance!.version()}:${JSON.stringify(parsed)}`)
        .digest('hex')
        .slice(0, 16);
      const srcDir = env.filePath ? path.dirname(env.filePath) : process.cwd();
      const svgPathLight = path.join(
        srcDir,
        `.vitepress/cache/markdown-hooks-graph/${hash}-light.svg`,
      );
      const svgPathDark = path.join(
        srcDir,
        `.vitepress/cache/markdown-hooks-graph/${hash}-dark.svg`,
      );

      // Generate SVG files if they don't exist
      if (!fs.existsSync(svgPathLight) || !fs.existsSync(svgPathDark)) {
        const svgDir = path.dirname(svgPathLight);
        if (!fs.existsSync(svgDir)) {
          fs.mkdirSync(svgDir, { recursive: true });
        }

        try {
          const dotLight = toDot(parsed, 'light');
          const dotDark = toDot(parsed, 'dark');
          const svgLight = graphvizInstance!.dot(dotLight, 'svg_inline');
          const svgDark = graphvizInstance!.dot(dotDark, 'svg_inline');

          fs.writeFileSync(svgPathLight, svgLight, 'utf8');
          fs.writeFileSync(svgPathDark, svgDark, 'utf8');
        } catch (error) {
          console.error('Error generating hooks-graph:', error);
          return '<div class="error">Error generating hooks-graph</div>';
        }
      }

      let svgLight = fs.readFileSync(svgPathLight, 'utf8');
      let svgDark = fs.readFileSync(svgPathDark, 'utf8');

      const applyAspectRatio = (svg: string): string => {
        const svgMatch = svg.match(/<svg\s+([^>]*)>/);
        if (svgMatch) {
          const attrs = svgMatch[1];
          const widthMatch = attrs.match(/width="([^"]*?)(?:pt)?"/);
          const heightMatch = attrs.match(/height="([^"]*?)(?:pt)?"/);
          const viewBoxMatch = attrs.match(/viewBox="([^"]*?)"/);

          if ((parsed.marginX || parsed.marginY) && viewBoxMatch) {
            const viewBox = viewBoxMatch[1];
            const parts = viewBox.split(/\s+/);
            if (parts.length === 4) {
              const marginXNum = +parsed.marginX || 0;
              const marginYNum = +parsed.marginY || 0;
              if (!Number.isNaN(marginXNum) || !Number.isNaN(marginYNum)) {
                const x = +parts[0];
                const y = +parts[1];
                const w = +parts[2] + marginXNum;
                const h = +parts[3] + marginYNum;
                svg = svg.replace(/viewBox="[^"]*"/, `viewBox="${x} ${y} ${w} ${h}"`);
              }
            }
          }
          if (widthMatch && heightMatch) {
            let width = +widthMatch[1];
            let height = +heightMatch[1];
            const marginXNum = +parsed.marginX || 0;
            const marginYNum = +parsed.marginY || 0;
            width += marginXNum;
            height += marginYNum;
            if (!Number.isNaN(width) && !Number.isNaN(height) && height > 0) {
              const aspectRatio = width / height;
              let styleStr = `aspect-ratio: ${aspectRatio};`;
              if (parsed.maxWidth) {
                styleStr += ` max-width: ${parsed.maxWidth};`;
              }
              svg = svg.replace(/(<svg\s+)/, `$1style="${styleStr}" `);
            }
          }
        }
        return svg.replace(/\s(?:height|width)="[^"]*"/g, '');
      };

      svgLight = applyAspectRatio(svgLight);
      svgDark = applyAspectRatio(svgDark);

      const legendLight = buildLegendTable(parsed, 'light');
      const legendDark = buildLegendTable(parsed, 'dark');
      if (legendLight) {
        svgLight = appendLegend(svgLight, legendLight, parsed.legendPosition);
      }
      if (legendDark) {
        svgDark = appendLegend(svgDark, legendDark, parsed.legendPosition);
      }

      return `<div class="hooks-graph-container light-only">${svgLight}</div><div class="hooks-graph-container dark-only">${svgDark}</div>`;
    }

    return defaultFenceRenderer(tokens, idx, options, env, self);
  };
}
