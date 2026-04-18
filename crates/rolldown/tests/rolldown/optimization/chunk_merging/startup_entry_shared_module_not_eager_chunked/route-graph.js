import { shared } from './route-shared.js';

const eagerValue = `${shared}-eager`;
const loadRouteComponent = () => import('./route-component.js');

export { eagerValue, loadRouteComponent };
