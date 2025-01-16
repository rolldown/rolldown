import { init } from '@module-federation/runtime';

const usedRemotes = __REMOTES__;
const usedShared = {};
const initRes = init({
    name: "mf-host",
    remotes: usedRemotes, 
    shared: usedShared,
    // plugins: [$runtimePlugin_0()],
    shareStrategy: 'version-first'
});

await Promise.all(await initRes.initializeSharing('default', {
    strategy: 'version-first',
    from: "build",
    initScope: []
}));