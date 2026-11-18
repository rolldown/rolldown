import { viteResolvePlugin } from 'rolldown/experimental';

const specialCharacters = '${(() => { throw new Error("interpolated") })()}`"\'\\\n\r\u2028\u2029';
export const browserModule = `node:test${specialCharacters}`;
export const peerDep = `peer${specialCharacters}`;
export const parentDep = `parent${specialCharacters}`;
export const peerModuleId = `__vite-optional-peer-dep:${peerDep}:${parentDep}`;

export function createResolver(root: string, isBundled: boolean, isProduction = false) {
  return viteResolvePlugin({
    resolveOptions: {
      isBuild: isBundled,
      isBundled,
      isProduction,
      asSrc: false,
      preferRelative: false,
      root,
      scan: false,
      mainFields: ['main'],
      conditions: [],
      externalConditions: [],
      extensions: ['.js'],
      tryIndex: true,
      preserveSymlinks: false,
      tsconfigPaths: false,
    },
    environmentConsumer: 'client',
    environmentName: 'client',
    builtins: [],
    external: [],
    noExternal: [],
    dedupe: [],
    legacyInconsistentCjsInterop: false,
    resolveSubpathImports: () => undefined,
  });
}
