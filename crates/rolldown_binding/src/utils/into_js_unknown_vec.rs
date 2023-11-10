use napi::{
  bindgen_prelude::{FromNapiValue, ToNapiValue},
  Env, JsUnknown,
};

pub trait IntoJsUnknownVec {
  fn into_js_unknown_vec(self, env: &Env) -> napi::Result<Vec<JsUnknown>>;
}

macro_rules! impl_tuple_to_vec {
    ( $( $parameter:ident ),+ $( , )* ) => {
      #[allow(unused_parens)]
      impl< $( $parameter ),+ > IntoJsUnknownVec for ( $( $parameter ),+, )
      where $( $parameter: ToNapiValue ),+
      {
        #[allow(non_snake_case)]
        fn into_js_unknown_vec(self, env: &Env) -> napi::Result<Vec<JsUnknown>> {
          let ( $( $parameter ),+, ) = self;
          let vec = unsafe {
            vec![
              $(
                JsUnknown::from_napi_value(
                  env.raw(),
                  ToNapiValue::to_napi_value(env.raw(), $parameter)?,
                )?
              ),+
            ]
          };
          Ok(vec)
        }
      }
    };
  }

impl_tuple_to_vec!(A);
impl_tuple_to_vec!(A, B);
impl_tuple_to_vec!(A, B, C);
impl_tuple_to_vec!(A, B, C, D);
impl_tuple_to_vec!(A, B, C, D, E);
impl_tuple_to_vec!(A, B, C, D, E, F);
impl_tuple_to_vec!(A, B, C, D, E, F, G);
impl_tuple_to_vec!(A, B, C, D, E, F, G, H);
impl_tuple_to_vec!(A, B, C, D, E, F, G, H, I);
impl_tuple_to_vec!(A, B, C, D, E, F, G, H, I, J);
