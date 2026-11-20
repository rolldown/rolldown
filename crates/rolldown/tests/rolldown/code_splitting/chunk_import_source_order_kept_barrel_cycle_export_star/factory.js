import { version } from './index.js';

export const getVersion = () => version;
export const createIcon = (name) => {
  (globalThis.loadOrder ??= []).push(name);
  return name;
};
