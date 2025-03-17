import { $ } from 'zx'
import { defineLoader, createMarkdownRenderer } from 'vitepress'
import nodePath from 'node:path'

const config = globalThis.VITEPRESS_CONFIG
const mdRender = await createMarkdownRenderer(
  config.srcDir,
  config.markdown,
  config.site.base,
  config.logger,
)

interface Input {
  name: string
  comment?: {
    summary: { kind: string; text: string }[]
  }
  children?: Input[]
  type?: {
    type?: (string & 'reflection') | 'array'
    declaration?: {
      children?: Input[]
    }
    elementType?: {
      type?: (string & 'reflection') | 'array'
      declaration?: Input
    }
  }
}

interface NormalizedItem {
  name: string
  jsdoc?: string
  children?: NormalizedItem[]
}

export interface OptionsDoc {
  inputOptions: NormalizedItem
  outputOptions: NormalizedItem
}

function normalizeDocJson(input: Input): NormalizedItem {
  if (input?.type?.type === 'reflection') {
    return {
      name: input.name,
      jsdoc: input.comment?.summary.map((x) => x.text).join('') ?? undefined,
      children:
        input.type.declaration?.children?.map(normalizeDocJson) ?? undefined,
    }
  } else if (input?.type?.type === 'array') {
    return {
      name: input.name,
      jsdoc: input.comment?.summary.map((x) => x.text).join('') ?? undefined,
      children:
        input.type.elementType?.declaration?.children?.map(normalizeDocJson) ??
        undefined,
    }
  } else {
    return {
      name: input.name,
      jsdoc: input.comment?.summary.map((x) => x.text).join('') ?? undefined,
      children: input.children?.map(normalizeDocJson) ?? undefined,
    }
  }
}

const repoRoot = nodePath.resolve(__dirname, '../..')

export default defineLoader({
  // FIXME: watch doesn't work
  watch: [nodePath.join(repoRoot, 'packages/rolldown/src/options/**.ts')],
  async load() {
    await $`pnpm run --filter rolldown extract-options-doc`
    const { default: docJson } = await import(
      // @ts-ignore - it doesn't exist in the first place, but it will be created by the above command
      '../../packages/rolldown/options-doc.json'.replace('', ''),
      // `.replace` is a workaround to disable compile-time loading done by vitepress
      { with: { type: 'json' } }
    )

    const normalized = normalizeDocJson(docJson)

    const output: OptionsDoc = {
      inputOptions: normalized
        .children!.find((x) => x.name === 'input-options')!
        .children!.find((x) => x.name === 'InputOptions')!,
      outputOptions: normalized
        .children!.find((x) => x.name === 'output-options')!
        .children!.find((x) => x.name === 'OutputOptions')!,
    }

    return mdRender.render(renderOptionsDocToMarkdown(output))
  },
})

function renderAncestorsTitle(ancestors: NormalizedItem[]): string {
  return ancestors
    .map((ancestor) => ancestor.name)
    .filter((name) => name != 'InputOptions' && name != 'OutputOptions')
    .join('.')
}

function renderNormalizedItemToMarkdown(
  item: NormalizedItem,
  ancestors: NormalizedItem[],
  level = 1,
): string {
  const ancestorsTitle = renderAncestorsTitle(ancestors)
  return [
    '#'.repeat(level) +
      ' ' +
      (ancestorsTitle
        ? `\`${ancestorsTitle}.${item.name}\``
        : `\`${item.name}\``),
    item.jsdoc ? `\n${ensureProperTitleLevel(item.jsdoc, level)}` : '',
    item.children
      ? item.children
          .map((child) =>
            renderNormalizedItemToMarkdown(
              child,
              [...ancestors, item],
              level + 1,
            ),
          )
          .join('\n')
      : '',
  ].join('\n')
}

function renderOptionsDocToMarkdown(item: OptionsDoc): string {
  return [
    renderNormalizedItemToMarkdown(item.inputOptions, [], 2),
    renderNormalizedItemToMarkdown(item.outputOptions, [], 2),
  ].join('\n')
}

const titleMarkRegex = /^(#+)/gm

/**
 * This function replaces `#` marks in the content with the proper number of `#` marks for the given level.
 * For example, if the content is `# Title`, and the level is 2, it will be replaced with `### Title`.
 */
function ensureProperTitleLevel(content: string, level: number) {
  let baseTitleMark = '#'.repeat(level)
  return content.replaceAll(titleMarkRegex, `${baseTitleMark}$1`)
}
