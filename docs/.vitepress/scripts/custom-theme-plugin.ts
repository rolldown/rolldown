// Reference: https://github.com/typedoc2md/typedoc-plugin-markdown/blob/typedoc-plugin-markdown%404.9.0/packages/typedoc-plugin-markdown/internal-docs/custom-theme.md

import { ReflectionKind, type Application, type Reflection } from 'typedoc';
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
      sources: (model, _options) => {
        if (!model.sources) return '';
        const sources = model.sources.map((source) => ({
          link: source.url,
          linkName: `${source.fileName}:${source.line}`,
        }));
        return `<DefinedIn :sources="${escapeAttr(JSON.stringify(sources))}" />`;
      },
      comment: (model, options) => {
        const result = superPartials.comment.call(this, model, options);
        // Remove the `**`Experimental`**` text that comes from `@experimental` tag
        return result.replace(/\*\*`Experimental`\*\*/g, '');
      },
      signatureTitle: (model, _options) => {
        const md: string[] = [];

        const params = (model.parameters || [])
          .map((param: any) => {
            const optional = param.flags?.isOptional ? '?' : '';
            const type = this.partials.someType(param.type);
            return `\`${param.name}${optional}\`: ${type}`;
          })
          .join(', ');

        const returnType = model.type ? this.partials.someType(model.type) : '`void`';

        md.push(`- **Type**: (${params}) => ${returnType}`);

        if (model.comment?.modifierTags?.has('@experimental')) {
          md.push('- **Experimental**');
        }

        return md.join('\n');
      },
      declarationTitle: (model) => {
        // https://github.com/typedoc2md/typedoc-plugin-markdown/blob/typedoc-plugin-markdown%404.9.0/packages/typedoc-plugin-markdown/src/theme/context/partials/member.declarationTitle.ts#L6
        const md: string[] = [];
        const declarationType = this.helpers.getDeclarationType(model);

        // Format type
        let typeStr: string;
        if (declarationType) {
          typeStr = this.partials.someType(declarationType);
        } else if (model.kind === ReflectionKind.TypeAlias) {
          const expandObjects = this.options.getValue('expandObjects');
          typeStr = expandObjects ? this.partials.declarationType(model) : '`object`';
        } else {
          typeStr = '`unknown`';
        }

        const type = declarationType || model.type;
        const isObjectWithChildren =
          type?.type === 'reflection' &&
          type.declaration?.children &&
          type.declaration.children.length > 0;

        if (isObjectWithChildren) {
          md.push('- **Type**: object with the properties below');
        } else {
          md.push(`- **Type**: ${typeStr}`);
        }

        if (model.flags?.isOptional) {
          md.push('- **Optional**');
        }

        if (
          model.defaultValue &&
          model.defaultValue !== '...' &&
          model.defaultValue !== model.name
        ) {
          md.push(`- **Default**: \`${model.defaultValue}\``);
        }

        if (model.comment?.modifierTags?.has('@experimental')) {
          md.push('- **Experimental**');
        }

        return md.join('\n');
      },
    };
  }
}

function escapeAttr(str: string) {
  return str.replace(/"/g, '&quot;');
}
