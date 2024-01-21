use rquickjs::{class::Trace, Class, Ctx};

use super::dom::{Node, DOM};
use crate::util::Result;

pub(super) fn register_builtin(ctx: &Ctx) -> Result<()> {
  Class::<DOM>::define(&ctx.globals())?;
  Class::<Node>::define(&ctx.globals())?;

  ctx
    .globals()
    .set("console", Class::instance(ctx.clone(), Console {})?)?;

  ctx
    .globals()
    .set("util", Class::instance(ctx.clone(), Util {})?)?;

  Ok(())
}

#[derive(Trace)]
#[rquickjs::class]
struct Console {}

#[rquickjs::methods]
impl Console {
  fn log(msg: String) {
    println!("[console.log] {}", msg);
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
