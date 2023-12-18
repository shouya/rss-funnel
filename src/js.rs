use std::collections::HashMap;

use rquickjs::class::Trace;
use rquickjs::markers::ParallelSend;
use rquickjs::{AsyncContext, Class, Ctx, FromJs};

use crate::util::{Error, Result};

pub struct Runtime {
  context: rquickjs::AsyncContext,
}

pub struct Globals {
  values: HashMap<String, String>,
}

impl Globals {
  pub fn new() -> Self {
    Self {
      values: HashMap::new(),
    }
  }

  pub fn set<T>(&mut self, key: &str, value: T)
  where
    T: serde::Serialize,
  {
    let json = serde_json::to_string(&value).unwrap();
    self.values.insert(key.to_string(), json);
  }

  fn set_ctx(self, ctx: &rquickjs::Ctx) -> Result<()> {
    for (key, value) in self.values {
      let val = ctx.json_parse(value)?;
      ctx.globals().set(key, val)?;
    }

    Ok(())
  }
}

impl Runtime {
  pub async fn new() -> Result<Self> {
    let runtime = rquickjs::AsyncRuntime::new()?;
    // limit memory usage to 32MB
    runtime.set_memory_limit(32 * 1024 * 1024).await;
    // limit max_stack_size to 1MB
    runtime.set_max_stack_size(1024 * 1024).await;

    let context = AsyncContext::full(&runtime).await?;
    context.with(|ctx| register_global_classes(&ctx)).await?;

    Ok(Self { context })
  }

  pub async fn eval<V>(&self, code: &str, globals: Globals) -> Result<V>
  where
    V: for<'js> FromJs<'js> + ParallelSend,
  {
    let code = code.to_string();
    self
      .context
      .with(|ctx: Ctx<'_>| -> Result<V> {
        globals.set_ctx(&ctx)?;

        let res = ctx.eval(code);
        if let Err(rquickjs::Error::Exception) = res {
          let exception = ctx.catch();
          let exception_repr =
            format!("{:?}", exception.as_exception().unwrap());
          return Err(Error::JsException(exception_repr));
        }

        Ok(res?)
      })
      .await
  }
}

fn register_global_classes(ctx: &Ctx) -> Result<()> {
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
