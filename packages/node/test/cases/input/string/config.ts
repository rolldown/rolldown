import type { RollupOptions } from '@rolldown/node'
import path from 'path'

const config: RollupOptions = {
  input: path.join(__dirname, 'main.js'),
}

export default config
