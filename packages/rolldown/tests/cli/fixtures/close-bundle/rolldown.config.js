import { defineConfig } from 'rolldown'

export default defineConfig({
  input: './index.js',
  plugins: [
    {
      name: 'test',
      closeBundle() {
        console.log('[test:closeBundle]')
      },
    },
  ],
})
