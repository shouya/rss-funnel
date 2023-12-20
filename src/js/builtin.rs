use rquickjs::{class::Trace, Class, Ctx, Module};

use crate::util::Result;

pub(super) fn register_builtin(ctx: &Ctx) -> Result<()> {
  ctx
    .globals()
    .set("console", Class::instance(ctx.clone(), Console {})?)?;

  Module::declare_def::<js_perf_hooks, _>(ctx.clone(), "perf_hooks")?;

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

#[rquickjs::module(rename_vars = "camelCase")]
mod perf_hooks {
  use rquickjs::class::Trace;

  #[derive(Trace)]
  #[rquickjs::class(rename = "performance")]
  struct Performance {}

  #[rquickjs::methods]
  impl Performance {
    #[qjs(static)]
    pub fn now() -> f64 {
      0.0
    }
  }
}
