use rquickjs::class::Trace;
use rquickjs::markers::ParallelSend;
use rquickjs::prelude::IntoArgs;
use rquickjs::{AsyncContext, Class, Ctx, FromJs, Function, IntoJs, Value};

use crate::util::{Error, Result};

pub struct Runtime {
  context: rquickjs::AsyncContext,
}

pub struct AsJson<T>(pub T);

impl<'js, T> IntoJs<'js> for AsJson<T>
where
  T: serde::Serialize,
{
  fn into_js(self, ctx: &Ctx<'js>) -> rquickjs::Result<Value<'js>> {
    let json = serde_json::to_string(&self.0).unwrap();
    ctx.json_parse(json)
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

  pub async fn set_global<T>(&self, key: &str, value: T)
  where
    T: for<'js> IntoJs<'js> + ParallelSend,
  {
    self
      .context
      .with(|ctx| {
        let val = value.into_js(&ctx).unwrap();
        ctx.globals().set(key, val).unwrap();
      })
      .await
  }

  pub async fn eval<V>(&self, code: &str) -> Result<V>
  where
    V: for<'js> FromJs<'js> + ParallelSend,
  {
    let code = code.to_string();
    self
      .context
      .with(|ctx: Ctx<'_>| -> Result<V> {
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

  pub async fn fn_exists(&self, name: &str) -> bool {
    self
      .context
      .with(|ctx| -> bool {
        let value: Result<Function<'_>, _> = ctx.globals().get(name);
        value.is_ok()
      })
      .await
  }

  pub async fn call_fn<V, Args>(&self, name: &str, args: Args) -> Result<V>
  where
    V: for<'js> FromJs<'js> + ParallelSend,
    Args: for<'js> IntoArgs<'js> + ParallelSend,
  {
    self
      .context
      .with(|ctx| -> Result<V> {
        let value: Result<Function<'_>, _> = ctx.globals().get(name);
        let Ok(fun) = value else {
          return Err(Error::JsException(format!(
            "function {} not found",
            name
          )));
        };

        let res = fun.call(args);

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
