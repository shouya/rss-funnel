use rquickjs::{class::Trace, Class, Ctx};

use crate::util::Result;

pub(super) fn register_builtin(ctx: &Ctx) -> Result<()> {
  ctx
    .globals()
    .set("console", Class::instance(ctx.clone(), Console {})?)?;

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
