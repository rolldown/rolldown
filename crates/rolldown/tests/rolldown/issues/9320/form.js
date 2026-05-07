import { actionImpl } from './action.js';
export function formImpl() {
  return 'form-result';
}
export function callActionFromForm() {
  return actionImpl();
}
