import { rows, t } from './lib.js';

const unused = {
  label: t('label'),
  read() {
    return rows;
  },
};
function defineComponent(value) {
  return value;
}
export const Editor = /* @__PURE__ */ defineComponent(unused);
