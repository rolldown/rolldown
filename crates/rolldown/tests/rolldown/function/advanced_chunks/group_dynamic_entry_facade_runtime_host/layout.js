import { dep } from './dep.cjs';
export default () => dep;
export const loadPage = () => import('./page.js');
