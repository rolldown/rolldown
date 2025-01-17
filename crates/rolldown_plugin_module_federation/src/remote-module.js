import { loadRemote } from '@module-federation/runtime';

let value = {};
let loaded = false;
module.exports = value;
module.exports.__mf__init__module__ = async () => {
    if (loaded) return;
    const remote = await loadRemote('__REMOTE__MODULE__ID__');
    Object.assign(value, remote);
    loaded = true;
}