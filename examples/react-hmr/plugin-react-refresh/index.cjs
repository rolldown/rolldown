// Copy from @vitejs/plugin-react-refresh

const fs = require('fs')
const { transformSync } = require('@babel/core')

const runtimePublicPath = '/@react-refresh'
const runtimeFilePath = require.resolve(
  'react-refresh/cjs/react-refresh-runtime.development.js',
)
const refreshEntry = 'react-refresh-entry.js'
// TODO the mix cjs and esm should be tranformed to cjs correctly
const runtimeCode = `
${fs.readFileSync(runtimeFilePath, 'utf-8')}
function debounce(fn, delay) {
  let handle
  return () => {
    clearTimeout(handle)
    handle = setTimeout(fn, delay)
  }
}
exports.performReactRefresh = debounce(exports.performReactRefresh, 16)
`

const preambleCode = `
import RefreshRuntime from "${runtimePublicPath}"
RefreshRuntime.injectIntoGlobalHook(window)
window.$RefreshReg$ = () => {}
window.$RefreshSig$ = () => (type) => type
window.__vite_plugin_react_preamble_installed__ = true
`

function reactRefreshPlugin() {
  let shouldSkip = false

  return {
    name: 'react-refresh',

    resolveId(id) {
      if (id === runtimePublicPath) {
        return id
      }
      if (id === refreshEntry) {
        return id
      }
    },

    load(id) {
      if (id === runtimePublicPath) {
        return runtimeCode
      }
      if (id === refreshEntry) {
        return preambleCode
      }
    },

    transform(code, id, ssr) {
      if (id === runtimePublicPath || id === refreshEntry) {
        return
      }
      if (!/\.(t|j)sx?$/.test(id) || id.includes('node_modules')) {
        return
      }

      // plain js/ts files can't use React without importing it, so skip
      // them whenever possible
      if (!id.endsWith('x') && !code.includes('react')) {
        return
      }

      const isReasonReact = id.endsWith('.bs.js')
      const result = transformSync(code, {
        presets: ['@babel/preset-react'],
        plugins: [
          require('@babel/plugin-syntax-import-meta'),
          [require('react-refresh/babel'), { skipEnvCheck: true }],
        ],
        ast: !isReasonReact,
        sourceMaps: true,
        sourceFileName: id,
      })

      if (!/\$RefreshReg\$\(/.test(result.code)) {
        // no component detected in the file
        return code
      }

      const header = `
  import RefreshRuntime from "${runtimePublicPath}";

  let prevRefreshReg;
  let prevRefreshSig;

  if (!window.__vite_plugin_react_preamble_installed__) {
    throw new Error(
      "vite-plugin-react can't detect preamble. Something is wrong" +
      "See https://github.com/vitejs/vite-plugin-react/pull/11#discussion_r430879201"
    );
  }

  if (import.meta.hot) {
    prevRefreshReg = window.$RefreshReg$;
    prevRefreshSig = window.$RefreshSig$;
    window.$RefreshReg$ = (type, id) => {
      RefreshRuntime.register(type, ${JSON.stringify(id)} + " " + id)
    };
    window.$RefreshSig$ = RefreshRuntime.createSignatureFunctionForTransform;
  }`.replace(/[\n]+/gm, '')

      const footer = `
  if (import.meta.hot) {
    window.$RefreshReg$ = prevRefreshReg;
    window.$RefreshSig$ = prevRefreshSig;

    ${
      isReasonReact || isRefreshBoundary(result.ast)
        ? `import.meta.hot.accept();`
        : ``
    }
    if (!window.__vite_plugin_react_timeout) {
      window.__vite_plugin_react_timeout = setTimeout(() => {
        window.__vite_plugin_react_timeout = 0;
        RefreshRuntime.performReactRefresh();
      }, 30);
    }
  }`

      return {
        code: `${header}${result.code}${footer}`,
        map: result.map,
      }
    },
  }
}

module.exports.preambleCode = preambleCode

/**
 * @param {import('@babel/core').BabelFileResult['ast']} ast
 */
function isRefreshBoundary(ast) {
  // Every export must be a React component.
  return ast.program.body.every((node) => {
    if (node.type !== 'ExportNamedDeclaration') {
      return true
    }
    const { declaration, specifiers } = node
    if (declaration && declaration.type === 'VariableDeclaration') {
      return declaration.declarations.every(
        ({ id }) => id.type === 'Identifier' && isComponentishName(id.name),
      )
    }
    return specifiers.every(
      ({ exported }) =>
        exported.type === 'Identifier' && isComponentishName(exported.name),
    )
  })
}

/**
 * @param {string} name
 */
function isComponentishName(name) {
  return typeof name === 'string' && name[0] >= 'A' && name[0] <= 'Z'
}

module.exports = reactRefreshPlugin
