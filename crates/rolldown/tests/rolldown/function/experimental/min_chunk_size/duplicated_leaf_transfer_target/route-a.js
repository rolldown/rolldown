import feature from './feature.cjs';
import { getLeaf } from './leaf.js';

export const value = `${feature.feature}:${getLeaf()}`;
