import { browser } from './esm-env.js'

export function render() {
  if (!browser) console.log('page-a')
}
