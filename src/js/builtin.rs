use rquickjs::{
  class::Trace,
  function::{Async, Func},
  Class, Ctx,
};

use super::dom::{Node, DOM};
use super::fetch::fetch;
use crate::util::Result;

pub(super) fn register_builtin(ctx: &Ctx) -> Result<(), rquickjs::Error> {
  Class::<DOM>::define(&ctx.globals())?;
  Class::<Node>::define(&ctx.globals())?;

  ctx
    .globals()
    .set("console", Class::instance(ctx.clone(), Console {})?)?;

  ctx
    .globals()
    .set("util", Class::instance(ctx.clone(), Util {})?)?;

  let fetch_fn = Func::new(Async(fetch));
  ctx.globals().set("fetch", fetch_fn)?;

  Ok(())
}

#[derive(Trace)]
#[rquickjs::class]
struct Console {}

#[rquickjs::methods]
impl Console {
  fn log(&self, value: rquickjs::Value<'_>) -> Result<(), rquickjs::Error> {
    let msg = match value.try_into_string() {
      Ok(s) => s.to_string()?,
      Err(v) => format!("[{}] {:?}", v.type_name(), v),
    };

    println!("[console.log] {}", msg);
    Ok(())
  }

  fn error(&self, value: rquickjs::Value<'_>) -> Result<(), rquickjs::Error> {
    let msg = match value.try_into_string() {
      Ok(s) => s.to_string()?,
      Err(v) => format!("[{}] {:?}", v.type_name(), v),
    };

    eprintln!("[console.error] {}", msg);
    Ok(())
  }
}

#[derive(Trace)]
#[rquickjs::class]
struct Util {}

#[rquickjs::methods]
impl Util {
  fn decode_html(html: String) -> Option<String> {
    htmlescape::decode_html(&html).ok()
  }

  fn encode_html(html: String) -> String {
    htmlescape::encode_minimal(&html)
  }
}
