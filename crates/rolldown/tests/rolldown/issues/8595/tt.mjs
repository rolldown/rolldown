import * as prettierParserBabel from './plugins/babel.mjs';
import * as prettierParserCss from './plugins/postcss.mjs';
import('./plugins/babel.mjs');
import('./plugins/flow.mjs');
import('./plugins/glimmer.mjs');
import('./plugins/html.mjs');
import('./plugins/postcss.mjs');
export const a = [
  import('./plugins/html.mjs'),
  import('./plugins/glimmer.mjs'),
  import('./plugins/meriyah.mjs'),
  import('./plugins/acorn.mjs'),
  import('./plugins/flow.mjs'),
];
import('@prettier/plugin-oxc');
import('@prettier/plugin-hermes');
import('prettier-plugin-astro');
import('prettier-plugin-marko');

export const options = [prettierParserCss, prettierParserBabel];
