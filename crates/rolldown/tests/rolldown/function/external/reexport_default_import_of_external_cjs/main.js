import { node_https } from './reexport.js';

export function getAgentCtor() {
	return node_https.Agent;
}
