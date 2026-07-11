import { existsSync, realpathSync } from 'node:fs';
import nodePath from 'node:path';

export function isBareDependency(source) {
  return (
    !source.startsWith('.') &&
    !source.startsWith('/') &&
    !source.startsWith('\0') &&
    !source.startsWith('$lib/') &&
    !nodePath.isAbsolute(source)
  );
}

export function createRegistryGraphResolver({ corpusDirectory, telemetry }) {
  const root = realpathSync(corpusDirectory);
  const libraryRoot = nodePath.join(root, 'docs/src/lib');
  const resolveCandidate = (candidate) => {
    if (existsSync(candidate)) return realpathSync(candidate);
    if (candidate.endsWith('.js')) {
      const typeScriptCandidate = `${candidate.slice(0, -3)}.ts`;
      if (existsSync(typeScriptCandidate)) {
        telemetry.nodeNextResolutions++;
        return realpathSync(typeScriptCandidate);
      }
    }
  };

  return {
    name: 'svelte-registry-graph-resolver',
    resolveId(source, importer) {
      if (source.startsWith('$lib/')) {
        telemetry.aliasRequests++;
        const candidate = nodePath.resolve(libraryRoot, source.slice('$lib/'.length));
        if (!candidate.startsWith(`${libraryRoot}${nodePath.sep}`)) {
          throw new Error(`$lib resolution escaped the project: ${source}`);
        }
        const resolved = resolveCandidate(candidate);
        if (!resolved) throw new Error(`unresolved $lib import ${source} from ${importer}`);
        telemetry.aliasResolutions++;
        return resolved;
      }
      if (source.startsWith('.') && importer && source.endsWith('.js')) {
        telemetry.relativeNodeNextRequests++;
        const resolved = resolveCandidate(nodePath.resolve(nodePath.dirname(importer), source));
        if (resolved) {
          telemetry.relativeNodeNextResolutions++;
          return resolved;
        }
      }
    },
  };
}
