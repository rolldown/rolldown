import { init } from '@module-federation/runtime';

__PLUGINS__

const usedRemotes = __REMOTES__;
const usedShared = {};
const initRes = init({
    name: __NAME__,
    remotes: usedRemotes, 
    shared: usedShared,
    plugins,
    shareStrategy: 'version-first'
});

await Promise.all(await initRes.initializeSharing('default', {
    strategy: 'version-first',
    from: "build",
    initScope: []
}));