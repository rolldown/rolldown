import { createHash } from 'node:crypto';
import { readFile, readdir, stat } from 'node:fs/promises';
import nodePath from 'node:path';
import vm from 'node:vm';

const VIRTUAL_ENTRY_PREFIX = '\0independent-vue-entry:';
const VUE_EXPORT_HELPER_ID = '\0/plugin-vue/export-helper';
const SCRIPT_EXTENSIONS = Object.freeze([
  '',
  '.js',
  '.mjs',
  '.cjs',
  '.ts',
  '.tsx',
  '.jsx',
  '.vue',
  '.json',
  '.graphql',
  '.graphqls',
  '.css',
  '.scss',
  '.less',
]);
const STYLE_EXTENSIONS = /\.(?:css|scss|sass|less|styl|stylus)$/;
const ASSET_EXTENSIONS = /\.(?:avif|bmp|eot|gif|ico|jpe?g|otf|pdf|png|svg|ttf|webp|woff2?)$/;
const TEXT_EXTENSIONS = /\.(?:graphqls?|md|txt)$/;
const STRUCTURED_TEXT_EXTENSIONS = /\.(?:ya?ml)$/;
const PROJECT_ROOT_TOKEN = '<project-root>';

const sha256 = (value) => createHash('sha256').update(value).digest('hex');
const byteSort = (left, right) => Buffer.compare(Buffer.from(left), Buffer.from(right));
const portable = (path) => path.split(nodePath.sep).join('/');

export function canonicalTransformResult(code, root) {
  if (typeof code !== 'string' || typeof root !== 'string' || root.length === 0) {
    throw new Error('transform-result canonicalization requires code and an absolute project root');
  }
  if (!nodePath.isAbsolute(root)) {
    throw new Error('transform-result canonicalization requires an absolute project root');
  }
  const canonicalCode = code.replaceAll(root, PROJECT_ROOT_TOKEN);
  return {
    bytes: Buffer.byteLength(canonicalCode),
    sha256: sha256(canonicalCode),
  };
}

async function pathKind(path) {
  try {
    const value = await stat(path);
    return value.isDirectory() ? 'directory' : value.isFile() ? 'file' : undefined;
  } catch {
    return undefined;
  }
}

async function resolveCandidate(candidate) {
  const [pathWithoutQuery, query = ''] = splitQuery(candidate);
  for (const extension of SCRIPT_EXTENSIONS) {
    const path = `${pathWithoutQuery}${extension}`;
    if ((await pathKind(path)) === 'file') return `${path}${query}`;
  }
  if (/\.[cm]?js$/.test(pathWithoutQuery)) {
    const withoutScriptExtension = pathWithoutQuery.replace(/\.[cm]?js$/, '');
    for (const extension of ['.ts', '.tsx']) {
      const path = `${withoutScriptExtension}${extension}`;
      if ((await pathKind(path)) === 'file') return `${path}${query}`;
    }
  }
  if ((await pathKind(pathWithoutQuery)) === 'directory') {
    const packagePath = nodePath.join(pathWithoutQuery, 'package.json');
    if ((await pathKind(packagePath)) === 'file') {
      const packageJson = JSON.parse(await readFile(packagePath, 'utf8'));
      for (const target of [packageJson.module, packageJson.main]) {
        if (typeof target === 'string') {
          const resolved = await resolveCandidate(nodePath.resolve(pathWithoutQuery, target));
          if (resolved) return `${splitQuery(resolved)[0]}${query}`;
        }
      }
    }
    for (const extension of SCRIPT_EXTENSIONS.slice(1)) {
      const indexPath = nodePath.join(pathWithoutQuery, `index${extension}`);
      if ((await pathKind(indexPath)) === 'file') return `${indexPath}${query}`;
    }
  }
}

function splitQuery(value) {
  const index = value.search(/[?#]/);
  return index === -1 ? [value, ''] : [value.slice(0, index), value.slice(index)];
}

async function walk(directory) {
  const entries = await readdir(directory, { withFileTypes: true });
  const nested = await Promise.all(
    entries.map((entry) => {
      const path = nodePath.join(directory, entry.name);
      if (entry.name === '.git' || entry.name === 'node_modules') return [];
      return entry.isDirectory() ? walk(path) : [path];
    }),
  );
  return nested.flat();
}

function executePinnedCommonJs(source, filename, require_) {
  const module = { exports: {} };
  const wrapper = vm.runInThisContext(
    `(function (exports, require, module, __filename, __dirname) {${source}\n})`,
    { filename },
  );
  wrapper(module.exports, require_, module, filename, nodePath.dirname(filename));
  return module.exports;
}

export async function generateGitLabEntries(root) {
  const helperPath = nodePath.join(root, 'config/webpack.helpers.js');
  const basePath = nodePath.join(root, 'config/helpers/entry_points.js');
  const [helperSource, baseSource] = await Promise.all([
    readFile(helperPath, 'utf8'),
    readFile(basePath, 'utf8'),
  ]);
  const pagesRoot = nodePath.join(root, 'app/assets/javascripts/pages');
  const pageEntries = (await walk(pagesRoot))
    .filter((path) => nodePath.basename(path) === 'index.js')
    .map((path) => portable(nodePath.relative(nodePath.join(root, 'app/assets/javascripts'), path)))
    .sort(byteSort);
  const webpackConstants = { IS_EE: false, IS_JH: false, ROOT_PATH: root };
  const helper = executePinnedCommonJs(helperSource, helperPath, (specifier) => {
    if (specifier === 'path') return nodePath;
    if (specifier === 'glob') {
      return {
        sync(pattern, options) {
          if (pattern !== 'pages/**/index.js') {
            throw new Error(`unexpected pinned GitLab entry glob: ${pattern}`);
          }
          const expectedCwd = nodePath.join(root, 'app/assets/javascripts');
          if (nodePath.resolve(options.cwd) !== expectedCwd) {
            throw new Error(`unexpected pinned GitLab entry cwd: ${options.cwd}`);
          }
          return pageEntries;
        },
      };
    }
    if (specifier === './webpack.constants') return webpackConstants;
    throw new Error(`unexpected pinned GitLab webpack.helpers dependency: ${specifier}`);
  });
  const base = executePinnedCommonJs(baseSource, basePath, (specifier) => {
    throw new Error(`unexpected pinned GitLab entry_points dependency: ${specifier}`);
  });
  const generated = helper.generateEntries(base.baseEntryPoints.default);
  const entries = { ...base.baseEntryPoints, ...generated.entries };
  return {
    entries,
    entriesState: generated.entriesState,
    pageEntryCount: pageEntries.length,
    totalEntryCount: Object.keys(entries).length,
    sourcePins: {
      webpackConfigSha256: sha256(await readFile(nodePath.join(root, 'config/webpack.config.js'))),
      webpackHelpersSha256: sha256(helperSource),
      entryPointsSha256: sha256(baseSource),
    },
  };
}

async function collectWorkspacePackages(root) {
  const packageFiles = [];
  for (const candidate of [nodePath.join(root, 'packages'), nodePath.join(root, 'sdk')]) {
    const kind = await pathKind(candidate);
    if (kind === 'directory') {
      packageFiles.push(
        ...(await walk(candidate)).filter((path) => nodePath.basename(path) === 'package.json'),
      );
    } else if (kind === 'file' && nodePath.basename(candidate) === 'package.json') {
      packageFiles.push(candidate);
    }
  }
  const sdkPackage = nodePath.join(root, 'sdk/package.json');
  if ((await pathKind(sdkPackage)) === 'file' && !packageFiles.includes(sdkPackage)) {
    packageFiles.push(sdkPackage);
  }
  const packages = [];
  for (const packagePath of packageFiles) {
    const content = await readFile(packagePath);
    const value = JSON.parse(content);
    if (typeof value.name === 'string') {
      packages.push({
        name: value.name,
        root: nodePath.dirname(packagePath),
        packageJson: value,
        packageJsonPath: portable(nodePath.relative(root, packagePath)),
        packageJsonSha256: sha256(content),
      });
    }
  }
  packages.sort(
    (left, right) =>
      right.name.length - left.name.length ||
      byteSort(left.name, right.name) ||
      byteSort(left.packageJsonPath, right.packageJsonPath),
  );
  const manifestEntries = [...packages]
    .sort(
      (left, right) =>
        byteSort(left.name, right.name) || byteSort(left.packageJsonPath, right.packageJsonPath),
    )
    .map(({ name, packageJsonPath, packageJsonSha256 }) => ({
      name,
      packageJsonPath,
      packageJsonSha256,
    }));
  return {
    packages,
    manifest: {
      count: manifestEntries.length,
      sha256: sha256(`${manifestEntries.map((entry) => JSON.stringify(entry)).join('\n')}\n`),
      entries: manifestEntries,
    },
  };
}

function exportTarget(packageJson, subpath) {
  const key = subpath ? `./${subpath}` : '.';
  const value = packageJson.exports?.[key];
  if (typeof value === 'string') return value;
  if (value && typeof value === 'object') {
    for (const condition of ['development', 'types', 'production', 'default']) {
      if (typeof value[condition] === 'string') return value[condition];
    }
  }
  if (!subpath) return packageJson.module ?? packageJson.main ?? './src/index.ts';
  return `./${subpath}`;
}

async function resolveWorkspaceImport(packages, source) {
  for (const package_ of packages) {
    if (source !== package_.name && !source.startsWith(`${package_.name}/`)) continue;
    const subpath = source === package_.name ? '' : source.slice(package_.name.length + 1);
    const target = exportTarget(package_.packageJson, subpath);
    if (target) {
      const resolved = await resolveCandidate(nodePath.resolve(package_.root, target));
      if (resolved) return resolved;
      for (const sourceTarget of [
        target.replace(/^\.\/dist\//, './src/').replace(/\.(?:c?js|mjs)$/, '.ts'),
        target.replace(/^\.\/dist\//, './').replace(/\.(?:c?js|mjs)$/, '.ts'),
      ]) {
        const sourceResolved = await resolveCandidate(
          nodePath.resolve(package_.root, sourceTarget),
        );
        if (sourceResolved) return sourceResolved;
      }
    }
    for (const fallback of subpath ? [`src/${subpath}`, `src/${subpath}/index`] : ['src/index']) {
      const resolved = await resolveCandidate(nodePath.join(package_.root, fallback));
      if (resolved) return resolved;
    }
    return;
  }
}

function aliasMatch(source, alias) {
  if (alias.endsWith('$')) return source === alias.slice(0, -1) ? '' : undefined;
  if (source === alias) return '';
  if (source.startsWith(`${alias}/`)) return source.slice(alias.length + 1);
}

async function resolveProjectAlias(project, root, source, workspacePackages) {
  const projectId = project.id;
  if (projectId === 'primevue') {
    const suffix = aliasMatch(source, 'primevue');
    if (suffix !== undefined) {
      const id = await resolveCandidate(nodePath.join(root, 'packages/primevue/src', suffix));
      if (id) return { id, kind: 'project-alias' };
    }
  }
  if (projectId === 'gitlab') {
    const javascriptRoot = nodePath.join(root, 'app/assets/javascripts');
    const aliases = [
      ['~', javascriptRoot],
      ['ee_else_ce', javascriptRoot],
      ['jh_else_ce', javascriptRoot],
      ['any_else_ce', javascriptRoot],
      ['vendor', nodePath.join(root, 'vendor/assets/javascripts')],
      ['shared_queries', nodePath.join(root, 'app/graphql/queries')],
      ['images', nodePath.join(root, 'app/assets/images')],
    ];
    for (const [alias, target] of aliases) {
      const suffix = aliasMatch(source, alias);
      if (suffix !== undefined) {
        const id = await resolveCandidate(nodePath.join(target, suffix));
        if (id) return { id, kind: 'project-alias' };
      }
    }
    if (source === '@gitlab/svgs/dist/icons.svg') {
      const id = await resolveCandidate(nodePath.join(javascriptRoot, 'lib/utils/icons_path.js'));
      if (id) return { id, kind: 'project-alias' };
    }
    if (source === '@gitlab/svgs/dist/illustrations.svg') {
      const id = await resolveCandidate(
        nodePath.join(javascriptRoot, 'lib/utils/illustrations_path.js'),
      );
      if (id) return { id, kind: 'project-alias' };
    }
  }
  if (projectId === 'vben') {
    const appPrefix = aliasMatch(source, '#');
    if (appPrefix !== undefined) {
      const id = await resolveCandidate(
        nodePath.join(root, project.appSourceRoot ?? 'apps/web-antd/src', appPrefix),
      );
      if (id) return { id, kind: 'project-alias' };
    }
    const id = await resolveWorkspaceImport(workspacePackages, source);
    if (id) return { id, kind: 'workspace-package' };
  }
  if (projectId === 'tdesign-amendment-candidate') {
    const sourcePrefix = aliasMatch(source, '@src');
    if (sourcePrefix !== undefined) {
      const id = await resolveCandidate(nodePath.join(root, 'packages/components', sourcePrefix));
      if (id) return { id, kind: 'project-alias' };
    }
    if (source === 'tdesign-vue-next') {
      const id = await resolveCandidate(nodePath.join(root, 'packages/components/index.ts'));
      if (id) return { id, kind: 'workspace-package' };
    }
    const id = await resolveWorkspaceImport(workspacePackages, source);
    if (id) return { id, kind: 'workspace-package' };
  }
  if (projectId === 'directus-amendment-candidate') {
    const appPrefix = aliasMatch(source, '@');
    if (appPrefix !== undefined) {
      const id = await resolveCandidate(nodePath.join(root, 'app/src', appPrefix));
      if (id) return { id, kind: 'project-alias' };
    }
    const id = await resolveWorkspaceImport(workspacePackages, source);
    if (id) return { id, kind: 'workspace-package' };
  }
}

function isBare(source) {
  return (
    !source.startsWith('.') &&
    !source.startsWith('/') &&
    !source.startsWith('\0') &&
    !/^[A-Za-z]:[\\/]/.test(source)
  );
}

function globRegex(pattern) {
  let source = '^';
  for (let index = 0; index < pattern.length; index++) {
    const character = pattern[index];
    if (character === '*' && pattern[index + 1] === '*') {
      index++;
      if (pattern[index + 1] === '/') {
        index++;
        source += '(?:.*/)?';
      } else {
        source += '.*';
      }
    } else if (character === '*') {
      source += '[^/]*';
    } else {
      source += character.replace(/[|\\{}()[\]^$+?.]/g, '\\$&');
    }
  }
  return new RegExp(`${source}$`);
}

async function expandImportMetaGlob(code, id, root, expansions) {
  const expression =
    /import\.meta\.glob(?:<[^>]+>)?\(\s*((?:['"][^'"]+['"])|(?:\[[^\]]+\]))\s*(?:,\s*(\{[\s\S]*?\}))?\s*\)/g;
  const imports = [];
  let changed = false;
  let serial = 0;
  let output = '';
  let cursor = 0;
  for (const match of code.matchAll(expression)) {
    const lineStart = code.lastIndexOf('\n', match.index - 1) + 1;
    const prefix = code.slice(lineStart, match.index).trimStart();
    if (prefix.startsWith('//') || prefix.startsWith('*')) continue;
    const patterns = [...match[1].matchAll(/['"]([^'"]+)['"]/g)].map((entry) => entry[1]);
    const options = match[2] ?? '';
    const fileSet = new Set();
    for (const pattern of patterns) {
      const firstWildcard = pattern.search(/\*/);
      const basePrefix = firstWildcard === -1 ? pattern : pattern.slice(0, firstWildcard);
      const baseDirectory = nodePath.resolve(nodePath.dirname(id), nodePath.dirname(basePrefix));
      const matcher = globRegex(pattern);
      for (const path of await walk(baseDirectory)) {
        const relative = `./${portable(nodePath.relative(nodePath.dirname(id), path))}`;
        if (matcher.test(relative)) fileSet.add(relative);
      }
    }
    const files = [...fileSet].sort(byteSort);
    const eager = /\beager\s*:\s*true\b/.test(options);
    const importMatch = options.match(/\bimport\s*:\s*['"]([^'"]+)['"]/);
    const queryMatch = options.match(/\bquery\s*:\s*['"]([^'"]+)['"]/);
    const properties = [];
    for (const file of files) {
      const specifier = `${file}${queryMatch?.[1] ?? ''}`;
      if (eager) {
        const binding = `__vueProjectGlob${serial++}`;
        if (importMatch?.[1] === 'default')
          imports.push(`import ${binding} from ${JSON.stringify(specifier)};`);
        else if (importMatch)
          imports.push(
            `import { ${importMatch[1]} as ${binding} } from ${JSON.stringify(specifier)};`,
          );
        else imports.push(`import * as ${binding} from ${JSON.stringify(specifier)};`);
        properties.push(`${JSON.stringify(file)}: ${binding}`);
      } else {
        const expression_ = `import(${JSON.stringify(specifier)})`;
        properties.push(
          `${JSON.stringify(file)}: () => ${importMatch ? `${expression_}.then((module) => module[${JSON.stringify(importMatch[1])}])` : expression_}`,
        );
      }
    }
    const record = {
      importer: portable(nodePath.relative(root, id)),
      sourceOffset: match.index,
      expressionSha256: sha256(match[0]),
      patterns,
      options: options.replace(/\s+/g, ' ').trim(),
      files,
    };
    const key = `${record.importer}\0${record.sourceOffset}`;
    const previous = expansions.get(key);
    if (previous && JSON.stringify(previous) !== JSON.stringify(record)) {
      throw new Error(`non-deterministic import.meta.glob expansion at ${record.importer}`);
    }
    expansions.set(key, record);
    output += code.slice(cursor, match.index) + `{${properties.join(',')}}`;
    cursor = match.index + match[0].length;
    changed = true;
  }
  if (!changed) return;
  output += code.slice(cursor);
  return { code: `${imports.join('\n')}\n${output}`, map: null };
}

export async function createGraphSupport(project, root, virtualEntries) {
  const workspace =
    project.id === 'vben' ||
    project.id === 'tdesign-amendment-candidate' ||
    project.id === 'directus-amendment-candidate'
      ? await collectWorkspacePackages(root)
      : { packages: [], manifest: { count: 0, sha256: sha256('\n'), entries: [] } };
  const externalEdges = new Map();
  const loaderKinds = new Map();
  const resolvedLocalAliases = new Map();
  const workspaceResolutionEdges = new Map();
  const globExpansions = new Map();
  const plugin = {
    name: 'independent-vue-project-graph-support',
    async resolveId(source, importer) {
      if (source === VUE_EXPORT_HELPER_ID || source.startsWith(VIRTUAL_ENTRY_PREFIX)) return source;
      if (source.startsWith('node:') || /^(?:data|https?):/.test(source)) {
        externalEdges.set(source, (externalEdges.get(source) ?? 0) + 1);
        return { id: source, external: true };
      }
      const aliased = await resolveProjectAlias(project, root, source, workspace.packages);
      if (aliased) {
        resolvedLocalAliases.set(source, (resolvedLocalAliases.get(source) ?? 0) + 1);
        if (aliased.kind === 'workspace-package') {
          const importerPath = importer ? splitQuery(importer)[0] : '<entry>';
          const record = {
            source,
            importer:
              importerPath === '<entry>'
                ? importerPath
                : portable(nodePath.relative(root, importerPath)),
            resolved: portable(nodePath.relative(root, splitQuery(aliased.id)[0])),
          };
          const key = JSON.stringify(record);
          workspaceResolutionEdges.set(key, (workspaceResolutionEdges.get(key) ?? 0) + 1);
        }
        return aliased.id;
      }
      if (isBare(source)) {
        if (
          (project.id === 'gitlab' && /\?vue3(?:$|&)/.test(source)) ||
          (project.id === 'gitlab' && source.includes('?vue3'))
        ) {
          throw new Error(`GitLab Vue 3 infection edge requires dual compiler routing: ${source}`);
        }
        externalEdges.set(source, (externalEdges.get(source) ?? 0) + 1);
        return { id: source, external: true };
      }
      if (!importer || source.startsWith('\0')) return;
      const [importerPath] = splitQuery(importer);
      const candidate = source.startsWith('/')
        ? source
        : nodePath.resolve(nodePath.dirname(importerPath), source);
      const resolved = await resolveCandidate(candidate);
      if (resolved) return resolved;
      throw new Error(
        `unresolved repository-local edge ${JSON.stringify(source)} from ${portable(nodePath.relative(root, importerPath))}`,
      );
    },
    async load(id) {
      if (id === VUE_EXPORT_HELPER_ID) {
        return `export default (sfc, props) => { const target = sfc.__vccOpts || sfc; for (const [key, value] of props) target[key] = value; return target; };`;
      }
      if (id.startsWith(VIRTUAL_ENTRY_PREFIX)) {
        const source = virtualEntries.get(id);
        if (source === undefined) throw new Error(`missing virtual entry source: ${id}`);
        return source;
      }
      const [path, query] = splitQuery(id);
      if (path.endsWith('.vue') && /(?:[?&])vue(?:&|$)/.test(query)) {
        if (/(?:[?&])type=style(?:&|$)/.test(query)) {
          loaderKinds.set(
            'vue-style-block-stub',
            (loaderKinds.get('vue-style-block-stub') ?? 0) + 1,
          );
          const isModule =
            /(?:[?&])lang\.module\./.test(query) || /(?:[?&])module(?:=|&|$)/.test(query);
          return {
            code: isModule ? 'export default {};' : 'export default "";',
            moduleType: 'js',
          };
        }
        if (/(?:[?&])type=custom(?:&|$)/.test(query)) {
          loaderKinds.set(
            'vue-custom-block-stub',
            (loaderKinds.get('vue-custom-block-stub') ?? 0) + 1,
          );
          return { code: 'export default undefined;', moduleType: 'js' };
        }
        throw new Error(`unsupported Vue child block in transform-only adapter: ${id}`);
      }
      if (query.includes('raw')) {
        const source = await readFile(path, 'utf8');
        loaderKinds.set('raw', (loaderKinds.get('raw') ?? 0) + 1);
        return { code: `export default ${JSON.stringify(source)};`, moduleType: 'js' };
      }
      if (STYLE_EXTENSIONS.test(path)) {
        loaderKinds.set('style-stub', (loaderKinds.get('style-stub') ?? 0) + 1);
        return { code: 'export default "";', moduleType: 'js' };
      }
      if (ASSET_EXTENSIONS.test(path)) {
        loaderKinds.set('asset-url-stub', (loaderKinds.get('asset-url-stub') ?? 0) + 1);
        return {
          code: `export default ${JSON.stringify(`/assets/${nodePath.basename(path)}`)};`,
          moduleType: 'js',
        };
      }
      if (TEXT_EXTENSIONS.test(path)) {
        const source = await readFile(path, 'utf8');
        loaderKinds.set('text-stub', (loaderKinds.get('text-stub') ?? 0) + 1);
        return { code: `export default ${JSON.stringify(source)};`, moduleType: 'js' };
      }
      if (STRUCTURED_TEXT_EXTENSIONS.test(path)) {
        const source = await readFile(path, 'utf8');
        loaderKinds.set('structured-text-stub', (loaderKinds.get('structured-text-stub') ?? 0) + 1);
        return { code: `export default ${JSON.stringify(source)};`, moduleType: 'js' };
      }
    },
    async transform(code, id) {
      const [path] = splitQuery(id);
      if (!/\.[cm]?[jt]sx?$/.test(path)) return;
      return expandImportMetaGlob(code, path, root, globExpansions);
    },
  };
  return {
    plugin,
    report() {
      const ordered = (map) =>
        Object.fromEntries([...map].sort(([left], [right]) => byteSort(left, right)));
      const globEntries = [...globExpansions.values()].sort((left, right) =>
        byteSort(
          `${left.importer}\0${left.sourceOffset}`,
          `${right.importer}\0${right.sourceOffset}`,
        ),
      );
      const workspaceEntries = [...workspaceResolutionEdges]
        .map(([value, calls]) => ({ ...JSON.parse(value), calls }))
        .sort((left, right) =>
          byteSort(
            `${left.importer}\0${left.source}\0${left.resolved}`,
            `${right.importer}\0${right.source}\0${right.resolved}`,
          ),
        );
      return {
        workspaceResolutionPolicy:
          project.id === 'vben'
            ? 'workspace source exports (development/types source target before unbuilt production dist)'
            : project.id === 'tdesign-amendment-candidate'
              ? 'pinned workspace package main/subpath source resolution; external packages remain external'
              : project.id === 'directus-amendment-candidate'
                ? 'pinned workspace package exports are mapped to checked-out source when unbuilt dist targets are absent; catalog packages remain external'
                : undefined,
        externalEdges: ordered(externalEdges),
        resolvedLocalAliases: ordered(resolvedLocalAliases),
        localLoaderReplacements: ordered(loaderKinds),
        globExpansionManifest: {
          count: globEntries.length,
          expandedFileCount: globEntries.reduce((total, entry) => total + entry.files.length, 0),
          sha256: sha256(`${globEntries.map((entry) => JSON.stringify(entry)).join('\n')}\n`),
          entries: globEntries,
        },
        workspacePackageManifest: workspace.manifest,
        workspaceResolutionManifest: {
          count: workspaceEntries.length,
          calls: workspaceEntries.reduce((total, entry) => total + entry.calls, 0),
          sha256: sha256(`${workspaceEntries.map((entry) => JSON.stringify(entry)).join('\n')}\n`),
          entries: workspaceEntries,
        },
      };
    },
  };
}

export async function createInputs(project, root) {
  const virtualEntries = new Map();
  let entryProvenance;
  if (project.entryGenerator === 'gitlab-production-generateEntries') {
    const generated = await generateGitLabEntries(root);
    const input = {};
    for (const [name, value] of Object.entries(generated.entries)) {
      const paths = (Array.isArray(value) ? value : [value]).map((path) =>
        nodePath.resolve(root, 'app/assets/javascripts', path),
      );
      const id = `${VIRTUAL_ENTRY_PREFIX}${encodeURIComponent(name)}`;
      virtualEntries.set(id, paths.map((path) => `import ${JSON.stringify(path)};`).join('\n'));
      input[name] = id;
    }
    entryProvenance = generated;
    return { input, virtualEntries, entryProvenance };
  }
  const entries = project.entries.map((path) => nodePath.join(root, path));
  const input =
    entries.length === 1
      ? entries[0]
      : Object.fromEntries(
          entries.map((path, index) => {
            const relative = project.entries[index];
            const name = portable(relative)
              .replace(/\.[^.]+$/, '')
              .replace(/[^A-Za-z0-9_-]+/g, '-');
            return [name, path];
          }),
        );
  return {
    input,
    virtualEntries,
    entryProvenance: {
      kind: 'real-source-entries',
      entries: project.entries,
      totalEntryCount: entries.length,
    },
  };
}

export function createAuditPlugins(root) {
  const sourceById = new Map();
  const resultById = new Map();
  const moduleIds = new Set();
  const sourceAudit = {
    name: 'independent-vue-source-audit',
    transform(code, id) {
      const [path, query] = splitQuery(id);
      if (query || !path.endsWith('.vue') || !nodePath.isAbsolute(path)) return;
      const relative = nodePath.relative(root, path);
      if (relative.startsWith('..') || nodePath.isAbsolute(relative)) {
        throw new Error(`reached SFC is outside the pinned project checkout: ${path}`);
      }
      const previous = sourceById.get(path);
      if (previous) previous.calls++;
      else {
        sourceById.set(path, {
          calls: 1,
          bytes: Buffer.byteLength(code),
          sha256: sha256(code),
        });
      }
    },
    moduleParsed(info) {
      moduleIds.add(info.id);
    },
  };
  const resultAudit = {
    name: 'independent-vue-result-audit',
    transform(code, id) {
      const [path, query] = splitQuery(id);
      if (query || !path.endsWith('.vue') || !sourceById.has(path)) return;
      const previous = resultById.get(path);
      if (previous) previous.calls++;
      else {
        resultById.set(path, {
          calls: 1,
          ...canonicalTransformResult(code, root),
        });
      }
    },
  };
  return {
    sourceAudit,
    resultAudit,
    report() {
      const paths = [...sourceById.keys()].sort((left, right) =>
        byteSort(portable(nodePath.relative(root, left)), portable(nodePath.relative(root, right))),
      );
      const transformHash = createHash('sha256');
      const sourceHash = createHash('sha256');
      let sourceBytes = 0;
      let resultBytes = 0;
      for (const path of paths) {
        const relative = portable(nodePath.relative(root, path));
        const source = sourceById.get(path);
        const result = resultById.get(path);
        transformHash.update(relative);
        transformHash.update('\0');
        transformHash.update(JSON.stringify(source));
        transformHash.update('\0');
        transformHash.update(JSON.stringify(result ?? null));
        transformHash.update('\n');
        sourceHash.update(`${relative}\0${source.bytes}\0${source.sha256}\n`);
        sourceBytes += source.bytes;
        resultBytes += result?.bytes ?? 0;
      }
      const normalizedModuleIds = [...moduleIds]
        .map((id) => id.replaceAll(root, '<project-root>'))
        .sort(byteSort);
      return {
        reachedSfcCount: paths.length,
        sourceCalls: [...sourceById.values()].reduce((total, value) => total + value.calls, 0),
        resultCalls: [...resultById.values()].reduce((total, value) => total + value.calls, 0),
        sourceBytes,
        resultBytes,
        exactOnce: paths.every(
          (path) => sourceById.get(path).calls === 1 && resultById.get(path)?.calls === 1,
        ),
        resultHashNormalization:
          'bytes and SHA-256 over transform code after exact absolute project root replacement with <project-root>',
        transformManifestSha256: transformHash.digest('hex'),
        reachedSourceManifestSha256: sourceHash.digest('hex'),
        reachedSfcPaths: paths.map((path) => portable(nodePath.relative(root, path))),
        graphModuleCount: normalizedModuleIds.length,
        graphModuleManifestSha256: sha256(`${normalizedModuleIds.join('\n')}\n`),
      };
    },
  };
}

export async function inspectGitLabCompilerContract(root, expectedContract) {
  const packageJson = JSON.parse(await readFile(nodePath.join(root, 'package.json'), 'utf8'));
  const sourcePins = {};
  for (const [relativePath, expectedSha256] of Object.entries(expectedContract.sourcePins)) {
    const actualSha256 = sha256(await readFile(nodePath.join(root, relativePath)));
    if (actualSha256 !== expectedSha256) {
      throw new Error(`GitLab compiler contract source drift: ${relativePath} ${actualSha256}`);
    }
    sourcePins[relativePath] = actualSha256;
  }
  const files = (await walk(nodePath.join(root, 'app/assets/javascripts')))
    .filter((path) => /\.[cm]?[jt]sx?$/.test(path))
    .sort((left, right) =>
      byteSort(portable(nodePath.relative(root, left)), portable(nodePath.relative(root, right))),
    );
  let explicitVue3InfectionEdges = 0;
  const examples = [];
  for (const path of files) {
    const source = await readFile(path, 'utf8');
    const matches = [...source.matchAll(/["']([^"']+\?vue3)["']/g)];
    explicitVue3InfectionEdges += matches.length;
    for (const match of matches.slice(0, Math.max(0, 10 - examples.length))) {
      examples.push({ path: portable(nodePath.relative(root, path)), source: match[1] });
    }
  }
  const configuredVersions = {
    vue2: packageJson.dependencies.vue,
    vueTemplateCompiler: packageJson.dependencies['vue-template-compiler'],
    vue3Compat: packageJson.dependencies['@vue/compat'],
    vue3CompilerSfc: packageJson.dependencies['@vue/compiler-sfc'],
    vue2Loader: packageJson.dependencies['vue-loader'],
    vue3Loader: packageJson.dependencies['vue-loader-vue3'],
  };
  const expectedVersions = {
    vue2: expectedContract.vue2,
    vueTemplateCompiler: expectedContract.vue2,
    vue3Compat: expectedContract.vue3Compat,
    vue3CompilerSfc: expectedContract.vue3Compat,
    vue2Loader: '15.11.1',
    vue3Loader: 'npm:vue-loader@17.4.2',
  };
  if (JSON.stringify(configuredVersions) !== JSON.stringify(expectedVersions)) {
    throw new Error(
      `GitLab compiler contract version drift: ${JSON.stringify(configuredVersions)}`,
    );
  }
  return {
    configuredVersions,
    sourcePins,
    explicitVue3InfectionEdges,
    examples,
    directAdapterCapability: {
      availableCompiler: '@vue/compiler-sfc 3.5.39 through unplugin-vue 7.2.0',
      supportsVue2LoaderSemantics: false,
      supportsVue3InfectionPropagation: false,
      supportsGitLabCustomElementCompilerSettings: false,
    },
  };
}
