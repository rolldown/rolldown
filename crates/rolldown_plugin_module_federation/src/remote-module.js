import { loadRemote } from '@module-federation/runtime';

let value = null
module.exports.__mf__init__module__ = async () => {
    if (value) return value;
    value = await loadRemote('__REMOTE__MODULE__ID__');
    return value;
}