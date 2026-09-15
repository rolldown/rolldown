import './first.js';
import { getPref, tag } from 'pref-pkg';

(globalThis.__events ??= []).push('main');

function boot() {
  return tag(getPref());
}

async function initApplication() {
  const pref = await getPref();
  return tag(pref);
}

globalThis.__result = boot();
globalThis.__ready = initApplication();
