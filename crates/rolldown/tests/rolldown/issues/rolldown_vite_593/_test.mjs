import { readFileSync, readdirSync } from 'fs'
import { dirname, resolve } from 'path'
import { fileURLToPath } from 'url'
import assert from 'assert'

const dir = resolve(dirname(fileURLToPath(import.meta.url)), 'dist')

function getStaticImports(filename) {
  const content = readFileSync(resolve(dir, filename), 'utf-8')
  const imports = []
  for (const match of content.matchAll(/from\s+["']\.\/(.+?)["']/g)) {
    imports.push(match[1])
  }
  return imports
}

function hasCircularDependency(graph, file, visited = new Set(), stack = new Set()) {
  if (stack.has(file)) return true
  if (visited.has(file)) return false
  visited.add(file)
  stack.add(file)
  for (const dep of graph.get(file) || []) {
    if (hasCircularDependency(graph, dep, visited, stack)) return true
  }
  stack.delete(file)
  return false
}

const files = readdirSync(dir).filter(f => f.endsWith('.js'))
const graph = new Map()
for (const file of files) {
  graph.set(file, getStaticImports(file))
}

for (const file of files) {
  assert(!hasCircularDependency(graph, file, new Set(), new Set()), `Circular dependency detected involving ${file}`)
}
