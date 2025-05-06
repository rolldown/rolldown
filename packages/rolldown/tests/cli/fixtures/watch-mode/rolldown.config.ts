import { defineConfig } from 'rolldown'

console.log('process.env.ROLLUP_WATCH', process.env.ROLLUP_WATCH)
console.log('process.env.ROLLDOWN_WATCH', process.env.ROLLDOWN_WATCH)

export default defineConfig({
  input: 'index.ts',
  cwd: import.meta.dirname,
  plugins: [
    {
      name: 'test',
      onLog() {
        console.log('onLog called:')
        console.log('this.meta.watchMode', this.meta.watchMode)
      },
      options() {
        console.log('options called:')
        console.log('this.meta.watchMode', this.meta.watchMode)
        console.log('process.env.ROLLUP_WATCH', process.env.ROLLUP_WATCH)
        console.log('process.env.ROLLDOWN_WATCH', process.env.ROLLDOWN_WATCH)

        this.info('trigger onLog')
      },
      buildStart() {
        console.log('buildStart called:')
        console.log('this.meta.watchMode', this.meta.watchMode)
      },
      closeBundle() {
        console.log('closeBundle called:')
        process.exit(0)
      },
    },
  ],
})
