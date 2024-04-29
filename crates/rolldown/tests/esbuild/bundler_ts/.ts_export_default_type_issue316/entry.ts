import dc_def, { bar as dc } from './keep/declare-class'
import dl_def, { bar as dl } from './keep/declare-let'
import im_def, { bar as im } from './keep/interface-merged'
import in_def, { bar as _in } from './keep/interface-nested'
import tn_def, { bar as tn } from './keep/type-nested'
import vn_def, { bar as vn } from './keep/value-namespace'
import vnm_def, { bar as vnm } from './keep/value-namespace-merged'

import i_def, { bar as i } from './remove/interface'
import ie_def, { bar as ie } from './remove/interface-exported'
import t_def, { bar as t } from './remove/type'
import te_def, { bar as te } from './remove/type-exported'
import ton_def, { bar as ton } from './remove/type-only-namespace'
import tone_def, { bar as tone } from './remove/type-only-namespace-exported'

export default [
	dc_def, dc,
	dl_def, dl,
	im_def, im,
	in_def, _in,
	tn_def, tn,
	vn_def, vn,
	vnm_def, vnm,

	i,
	ie,
	t,
	te,
	ton,
	tone,
]