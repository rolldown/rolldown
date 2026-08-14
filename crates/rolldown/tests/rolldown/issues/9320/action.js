import { formImpl } from './form.js';
export function actionImpl() {
  return 'action-result';
}
export function callFormFromAction() {
  return formImpl();
}
