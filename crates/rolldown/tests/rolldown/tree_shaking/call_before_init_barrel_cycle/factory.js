import { version } from './index.js';
import { register } from './register.js';

export const getVersion = () => version;
export const createIcon = /* @__NO_SIDE_EFFECTS__ */ (name) => register(name);
