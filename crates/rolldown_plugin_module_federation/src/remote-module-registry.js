import { loadRemote } from "@module-federation/runtime";

const registry = {}
const loading = {}

export async function loadRemoteToRegistry(id) {
    if (!registry[id] && !loading[id]) {
        loading[id] = true
        registry[id] = await loadRemote(id)
        delete loading[id]
    }
}

export function getModuleFromRegistry(id) {
    const module = registry[id]
    if (!module) {
        throw new Error(`Module ${id} not found in registry`)
    }
    return module
}