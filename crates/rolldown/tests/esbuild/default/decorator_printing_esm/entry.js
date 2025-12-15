import { constant } from './constants'
import { imported } from 'somewhere'
import { undef } from './empty'

_ = class Outer {
	#bar;

	classes = [
		class { @imported @imported() imported },
		class { @unbound @unbound() unbound },
		class { @constant @constant() constant },
		class { @undef @undef() undef },

		class { @(element[access]) indexed },
		class { @foo.#bar private },
		class { @foo.\u30FF unicode },
		class { @(() => {}) arrow },
	]
}