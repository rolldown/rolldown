import { snakeCase } from 'change-case'
import markdown from 'markdown-it'
import assert from 'node:assert'
export function parseEsbuildSnap(source: string) {
  let cases = source.split(
    '================================================================================',
  )
  return cases.map(parseEsbuildCase)
}

function parseEsbuildCase(source: string): {
  name: string
  sourceList: { name: string; content: string }[]
} {
  let lines = source.trimStart().split('\n')
  let [name, ...rest] = lines
  let trimmedName = name.slice(4)
  let normalizedName = snakeCase(trimmedName)
  let content = rest.join('\n')
  return { name: normalizedName, sourceList: parseContent(content) }
}

function parseContent(content: string) {
  // Define a regex pattern to match the filename and its content
  const regex = /----------\s*(.+?)\s*----------\s*([\s\S]*?)(?=----------|$)/g

  const result = []
  let match

  // Use regex to find all matches in the input
  while ((match = regex.exec(content)) !== null) {
    const filename = match[1].trim() // Extract the filename
    const content = match[2].trim() // Extract the content

    // Push an object with filename and content into the result array
    result.push({
      name: filename,
      content: content,
    })
  }

  return result
}

export function parseRolldownSnap(source: string | undefined) {
  if (!source) {
    return undefined
  }
  let match
  // strip `---source---` block
  while ((match = /---\n([\s\S]+?)\n---/.exec(source))) {
    source = source.slice(match.index + match[0].length)
  }
  // default mode
  const md = markdown()
  let tokens = md.parse(source, {})
  let i = 0
  let ret = []
  while (i < tokens.length) {
    let token = tokens[i]

    if (token.type === 'heading_open' && token.tag === 'h2') {
      let headingToken = tokens[i + 1]
      assert(headingToken.type === 'inline')
      let filename = headingToken.content
      let codeToken = tokens[i + 3]
      assert(codeToken.tag === 'code')
      let content = codeToken.content
      ret.push({ filename, content })
      i += 3
    }
    i++
  }
  return ret
}
