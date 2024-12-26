import { $ } from 'zx'

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

export default {
  async load() {
    await $`pnpm run --filter rolldown extract-options-doc`
    const { default: docJson } = await import(
      // @ts-ignore - it doesn't exist in the first place, but it will be created by the above command
      '../../packages/rolldown/options-doc.json'.replace('', ''),
      // `.replace` is a workaround to disable compile-time loading done by vitepress
      { assert: { type: 'json' } }
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

    return JSON.stringify(output, null, 2)
  },
}
