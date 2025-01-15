import { init as runtimeInit } from '@module-federation/runtime';

const usedRemotes = []
const usedShared = {}

const exposesMap = __EXPOSES_MAP__

export function get(moduleName) {
    if (!(moduleName in exposesMap))
        throw new Error(`Module ${moduleName} does not exist in container.`)
    return exposesMap[moduleName]()
}

const initTokens = {}
const shareScopeName = "default"

export async function init(shared={}, initScope=[]) {
    const initRes = runtimeInit({
        name: "mf-remote",
        remotes: usedRemotes,
        shared: usedShared,
        shareStrategy: 'version-first'
    });
    // handling circular init calls
    var initToken = initTokens[shareScopeName];
    if (!initToken)
        initToken = initTokens[shareScopeName] = {
            from: mfName
        };
    if (initScope.indexOf(initToken) >= 0)
        return;
    initScope.push(initToken);
    initRes.initShareScopeMap('default', shared);
    await Promise.all(await initRes.initializeSharing('default', {
        strategy: 'version-first',
        from: "build",
        initScope
    }));
    return initRes
}