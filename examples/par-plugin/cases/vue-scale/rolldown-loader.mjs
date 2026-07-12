import nodePath from 'node:path';
import { pathToFileURL } from 'node:url';

const packageRoot = process.env.ROLLDOWN_RESEARCH_PACKAGE_ROOT;
if (!packageRoot || !nodePath.isAbsolute(packageRoot)) {
  throw new Error('ROLLDOWN_RESEARCH_PACKAGE_ROOT must identify an absolute Rolldown package');
}
const mappings = {
  rolldown: 'index.mjs',
  'rolldown/experimental': 'experimental-index.mjs',
  'rolldown/parallelPlugin': 'parallel-plugin.mjs',
};

export function resolve(specifier, context, nextResolve) {
  const fileName = mappings[specifier];
  if (!fileName) return nextResolve(specifier, context);
  return {
    shortCircuit: true,
    url: pathToFileURL(nodePath.join(packageRoot, 'dist', fileName)).href,
  };
}
