import { defineTest } from '@tests'
import { expect } from 'vitest'
import nodePath from 'node:path'

export default defineTest({
  config: {
    inject: {
      // import { Promise } from './promise-shim'
      Promise: ['./promise-shim', 'Promise'],
      // import { Promise as P } from './promise-shim'
      P: ['./promise-shim', 'Promise'],
      // import $ from 'jquery'
      $: './jquery',
      // import * as fs from 'node:fs'
      fs: ['./node-fs', '*'],
      'Object.assign': './object-assign-shim',
    },
    external: ['node:assert'],
  },
  afterTest: function (output) {
    expect(output.output[0].code).toMatchFileSnapshot(
      nodePath.join(import.meta.dirname, 'output.snap'),
    )
  },
})
