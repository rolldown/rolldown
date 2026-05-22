import './vue.js';
import { sharedState } from './shared.js';

export const get = () => sharedState.touched;
