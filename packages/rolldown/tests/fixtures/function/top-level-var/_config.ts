import { expect } from 'vitest'
import { defineTest } from 'rolldown-tests'

export default defineTest({
    config: {
        output: {
            topLevelVar: true,
        },
    },
    afterTest(output) {
        expect(output.output[0].code.includes('var greeting')).toBe(true)
    },
})
