import { createRouter } from './router.mjs'

const router = createRouter(() => import('./page.mjs'))

await router.isReady

globalThis.result.push('ready');

