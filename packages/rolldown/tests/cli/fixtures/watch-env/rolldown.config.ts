import { expect } from 'vitest'

export default {
  input: 'index.ts',
  cwd: import.meta.dirname,
  plugins: [
    {
      name: 'test',
      options() {
        expect(process.env.ROLLUP_WATCH).toBe('true')
        expect(process.env.ROLLDOWN_WATCH).toBe('true')
      },
    },
  ],
}
