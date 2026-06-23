// JS-side @vue/compiler-sfc wrapper used by utils-sync and utils-async variants.
//
// Stitches parse → compileScript → compileTemplate into a single JS module so
// the (id, code) -> code shape matches the React Compiler bench's
// `utilsTransformSync` / `utilsTransform` helpers from rolldown/utils.
//
// Style compilation is skipped — Vize doesn't emit CSS either, so this keeps
// the comparison fair.

import {
  compileScript,
  compileTemplate,
  parse,
  rewriteDefault,
} from '@vue/compiler-sfc';

let uid = 0;
function nextScopeId() {
  return `data-v-${(uid++).toString(36)}`;
}

export function compileVueSync(id, code) {
  const { descriptor, errors } = parse(code, { filename: id });
  if (errors.length) throw errors[0];

  const scopeId = nextScopeId();
  const hasScript = !!(descriptor.script || descriptor.scriptSetup);
  const lang = descriptor.scriptSetup?.lang ?? descriptor.script?.lang;
  const isTs = lang === 'ts' || lang === 'tsx';
  const babelParserPlugins = isTs ? ['typescript'] : [];

  const script = hasScript
    ? compileScript(descriptor, {
      id: scopeId,
      inlineTemplate: false,
      babelParserPlugins,
    })
    : { content: 'export default {}', bindings: undefined };

  let templateCode = '';
  if (descriptor.template) {
    const t = compileTemplate({
      id: scopeId,
      filename: id,
      source: descriptor.template.content,
      compilerOptions: {
        bindingMetadata: script.bindings,
        expressionPlugins: isTs ? ['typescript'] : [],
      },
    });
    if (t.errors.length) throw t.errors[0];
    templateCode = t.code.replace(
      /export (function|const) (render|ssrRender)/,
      '$1 _sfc_$2',
    );
  }

  // rewriteDefault uses its own babel parse; without parserPlugins it'll
  // choke on `import type { ... }` and other TS syntax that compileScript
  // leaves intact.
  const rewritten = rewriteDefault(
    script.content,
    '_sfc_main',
    babelParserPlugins,
  );
  const code_ =
    rewritten +
    '\n' +
    templateCode +
    '\n' +
    `_sfc_main.render = _sfc_render;\n` +
    `export default _sfc_main;\n`;
  return { code: code_ };
}

export async function compileVueAsync(id, code) {
  // Matches utilsTransform's shape from the React Compiler bench: the JS hook
  // is async, but the underlying compile is sync — await Promise.resolve() so
  // the microtask boundary is observable.
  await Promise.resolve();
  return compileVueSync(id, code);
}
