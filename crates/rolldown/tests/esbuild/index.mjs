import * as path from 'path'
import * as fs from 'fs'


const categoryPath = path.resolve(import.meta.dirname, "default")
const list = fs.readdirSync(categoryPath)

for (let item of list) {
  const testPath = path.resolve(categoryPath, item, "_config.json")
  if (fs.existsSync(testPath)) {
    const json = JSON.parse(fs.readFileSync(testPath, "utf-8"))
    json.expectExecuted = false
    fs.writeFileSync(testPath, JSON.stringify(json, null, 2))
  }
}


