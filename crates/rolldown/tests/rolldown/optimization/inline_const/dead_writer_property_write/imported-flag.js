import { registry } from './registry.js';

// Tree shaking finds `registry.enable = ...` from the local `registry` binding, not from the
// symbol in `registry.js`.
export let imported = false;
registry.enable = () => {
  imported = true;
};

export function run_imported() {
  registry.enable();
}
