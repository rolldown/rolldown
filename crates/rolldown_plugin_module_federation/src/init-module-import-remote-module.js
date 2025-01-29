import { loadRemoteToRegistry } from 'mf:remote-module-registry.js'

const remoteModules = __REMOTE__MODULES__

await Promise.all(remoteModules.map((module) => loadRemoteToRegistry(module)))