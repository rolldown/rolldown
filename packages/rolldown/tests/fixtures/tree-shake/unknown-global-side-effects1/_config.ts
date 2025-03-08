import { defineTest } from 'rolldown-tests'
import { expect } from 'vitest'

export default defineTest({
  afterTest: (output) => {
    let code = output.output[0].code

    expect(code).toMatchInlineSnapshot(`
			"
			//#region main.js
			const jQuery = $;
			const element = angular.element;

			//#endregion"
		`)
  },
})
