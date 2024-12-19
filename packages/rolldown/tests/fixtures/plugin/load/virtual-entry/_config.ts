import { defineTest } from '@tests'
import path from 'node:path'
import { expect } from 'vitest'

const entryName = 'virtual:entry'
const idList: string[] = []

export default defineTest({
  config: {
    input: {
      main: entryName,
    },
    plugins: [
      {
        name: 'virtual-entry',
        resolveId(source) {
          if (source === entryName) {
            return source
          }
        },
        load(id) {
          if (id === entryName) {
            return `
              import * as lib from "./main.js";
              console.log(lib);
            `
          }
        },
        transform(_, id) {
          idList.push(id)
        },
      },
    ],
  },
  beforeTest: () => {
    idList.length = 0
  },
  afterTest: () => {
    expect(idList).toStrictEqual([entryName, path.join(__dirname, './main.js')])
  },
})
