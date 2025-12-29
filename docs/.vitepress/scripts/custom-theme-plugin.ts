// Reference: https://github.com/typedoc2md/typedoc-plugin-markdown/blob/typedoc-plugin-markdown%404.9.0/packages/typedoc-plugin-markdown/internal-docs/custom-theme.md

import type { Application, Reflection } from 'typedoc';
import {
  type MarkdownPageEvent,
  MarkdownTheme,
  MarkdownThemeContext,
} from 'typedoc-plugin-markdown';

export function load(app: Application) {
  app.renderer.defineTheme('customTheme', CustomTheme);
}

class CustomTheme extends MarkdownTheme {
  getRenderContext(page: MarkdownPageEvent<Reflection>) {
    return new CustomThemeContext(this, page, this.application.options);
  }
}

class CustomThemeContext extends MarkdownThemeContext {
  constructor(theme: MarkdownTheme, page: MarkdownPageEvent<Reflection>, options: any) {
    super(theme, page, options);
    const superPartials = this.partials;

    this.partials = {
      ...superPartials,
      // Use DefinedIn component for "Defined in: [source](link)"
      sources(model, _options) {
        if (!model.sources) return '';
        const sources = model.sources.map((source) => ({
          link: source.url,
          linkName: `${source.fileName}:${source.line}`,
        }));
        return `<DefinedIn :sources="${escapeAttr(JSON.stringify(sources))}" />`;
      },
      declarationTitle(model) {
        // TODO: improve this to output a better formatted type info
        // https://github.com/typedoc2md/typedoc-plugin-markdown/blob/typedoc-plugin-markdown%404.9.0/packages/typedoc-plugin-markdown/src/theme/context/partials/member.declarationTitle.ts#L6
        return superPartials.declarationTitle.call(this, model).replace('>', '- **Type**:');
      },
    };
  }
}

function escapeAttr(str: string) {
  return str.replace(/"/g, '&quot;');
}
