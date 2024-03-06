mod builtin;
mod dom;
mod fetch;

use std::fs;
use std::path::PathBuf;

use rquickjs::loader::{
  BuiltinLoader, BuiltinResolver, FileResolver, Loader, Resolver, ScriptLoader,
};
use rquickjs::markers::ParallelSend;
use rquickjs::module::ModuleData;
use rquickjs::prelude::IntoArgs;
use rquickjs::promise::Promise;
use rquickjs::{
  async_with, AsyncContext, Ctx, FromJs, Function, IntoJs, Module, Value,
};
use url::Url;

use crate::util::JsError;

pub struct Runtime {
  context: rquickjs::AsyncContext,
}

#[derive(Default, Clone, Debug, PartialEq, Eq, Hash)]
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

impl<'js, T> FromJs<'js> for AsJson<T>
where
  T: serde::de::DeserializeOwned,
{
  fn from_js(ctx: &Ctx<'js>, value: Value<'js>) -> rquickjs::Result<Self> {
    let json = ctx
      .json_stringify(value)?
      .and_then(|s| s.to_string().ok())
      .unwrap_or_else(|| "null".to_string());

    let value = serde_json::from_str(&json).map_err(|e| {
      let type_name = std::any::type_name::<T>();
      let message = format!("{}: {}", e, json);
      rquickjs::Error::new_from_js_message("json", type_name, message)
    })?;
    Ok(Self(value))
  }
}

impl Runtime {
  pub async fn new() -> Result<Self, JsError> {
    let runtime = rquickjs::AsyncRuntime::new()?;
    // limit memory usage to 32MB
    runtime.set_memory_limit(32 * 1024 * 1024).await;
    // limit max_stack_size to 1MB
    runtime.set_max_stack_size(1024 * 1024).await;

    let resolver = (
      BuiltinResolver::default(),
      RemoteResolver,
      FileResolver::default(),
    );
    let remote_loader = RemoteLoader::default()
      .with_cache_dir(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".cache"));
    let loader = (
      BuiltinLoader::default(),
      remote_loader,
      ScriptLoader::default(),
    );
    runtime.set_loader(resolver, loader).await;

    let context = AsyncContext::full(&runtime).await?;
    context.with(|ctx| builtin::register_builtin(&ctx)).await?;

    Ok(Self { context })
  }

  #[allow(unused)]
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

  // return exported names
  #[allow(unused)]
  pub async fn eval_module(
    &self,
    name: &str,
    code: &str,
  ) -> Result<Vec<String>, JsError> {
    let code = code.to_string();

    let mut names = Vec::new();
    self
      .context
      .with(|ctx: Ctx<'_>| -> Result<(), JsError> {
        let module = Module::evaluate(ctx.clone(), name, code);

        if let Err(rquickjs::Error::Exception) = module {
          let exception = ctx.catch();
          let exception_repr =
            format!("{:?}", exception.as_exception().unwrap());
          return Err(JsError::Exception(exception_repr));
        }

        let globals = ctx.globals();

        for item in module?.entries() {
          let (name, value): (String, Value) = item?;
          globals.set(&name, value)?;
          names.push(name);
        }

        Ok(())
      })
      .await?;

    // self.context.runtime().execute_pending_job().await.ok();
    Ok(names)
  }

  pub async fn eval<V>(&self, code: &str) -> Result<V, JsError>
  where
    V: for<'js> FromJs<'js> + ParallelSend,
  {
    let code = code.to_string();

    self
      .context
      .with(|ctx: Ctx<'_>| -> Result<V, JsError> {
        let res = ctx.eval(code);

        if let Err(rquickjs::Error::Exception) = res {
          let exception = ctx.catch();
          let exception_repr =
            format!("{:?}", exception.as_exception().unwrap());
          return Err(JsError::Exception(exception_repr));
        }

        Ok(res?)
      })
      .await
  }

  pub async fn fn_exists(&self, name: &str) -> bool {
    // self.context.runtime().execute_pending_job().await.ok();
    self
      .context
      .with(|ctx| -> bool {
        let value: Result<Function<'_>, _> = ctx.globals().get(name);
        value.is_ok()
      })
      .await
  }

  /// Automatically detect if the function is async and wait for the result
  pub async fn call_fn<V, Args>(
    &self,
    name: &str,
    args: Args,
  ) -> Result<V, JsError>
  where
    V: for<'js> FromJs<'js> + ParallelSend + 'static,
    Args: for<'js> IntoArgs<'js> + ParallelSend,
  {
    // self.context.runtime().execute_pending_job().await.ok();

    async_with!(self.context => |ctx| {
      let value: Result<Function<'_>, _> = ctx.globals().get(name);
      let Ok(fun) = value else {
        return Err(JsError::Exception(format!("function {} not found", name)));
      };

      let is_async: bool = ctx.eval(format!("{name}[Symbol.toStringTag] === 'AsyncFunction'"))?;

      let val = if is_async {
        match fun.call::<_, Promise<_>>(args) {
          Ok(promise) => V::from_js(&ctx, promise.await?),
          Err(e) => Err(e),
        }
      } else {
        fun.call::<_, V>(args)
      };

      match val {
        Ok(v) => Ok(v),
        Err(rquickjs::Error::Exception) => {
          let exception = ctx.catch();
          let exception_repr = format!("{:?}", exception.as_exception().unwrap());
          return Err(JsError::Exception(exception_repr));
        }
        Err(e) => {
          return Err(JsError::from(e));
        }
      }
    })
    .await
  }
}

struct RemoteResolver;

impl Resolver for RemoteResolver {
  fn resolve(
    &mut self,
    _ctx: &Ctx,
    base: &str,
    name: &str,
  ) -> rquickjs::Result<String> {
    let is_remote =
      |s: &str| s.starts_with("http://") || s.starts_with("https://");
    if is_remote(name) {
      return Ok(name.to_string());
    }

    let abs_url = Url::parse(base)
      .and_then(|base| base.join(name))
      .map_err(|_| rquickjs::Error::new_loading(name))
      .map(|url| url.to_string())?;

    Ok(abs_url)
  }
}

struct RemoteLoader {
  cache_dir: Option<PathBuf>,
  extensions: Vec<String>,
}

impl Default for RemoteLoader {
  fn default() -> Self {
    Self {
      cache_dir: None,
      extensions: vec!["js".to_string()],
    }
  }
}

impl RemoteLoader {
  pub fn with_cache_dir(mut self, cache_dir: PathBuf) -> Self {
    self.cache_dir = Some(cache_dir);
    self
  }

  fn name_valid(&self, name: &str) -> bool {
    self.extensions.iter().any(|ext| name.ends_with(ext))
      && name.starts_with("http://")
      || name.starts_with("https://")
  }

  fn cache_file_name(&self, name: &str) -> PathBuf {
    let mut uri = PathBuf::from(name);
    let ext = uri
      .extension()
      .and_then(|s| s.to_str())
      .unwrap_or("")
      .to_string();
    uri.set_extension("");
    let digest = uri_to_hash(uri);
    let Some(cache_dir) = self.cache_dir.as_ref() else {
      panic!("cache_file_name assumes cache_dir is set");
    };

    let file_name = format!("{}.{}", digest, ext);
    cache_dir.join(file_name)
  }

  fn try_load(&self, name: &str) -> rquickjs::Result<String> {
    let err = rquickjs::Error::new_loading(name);
    if !self.name_valid(name) {
      return Err(err);
    };

    if let Some(script) = self.try_load_cache(name) {
      return Ok(script);
    }

    match self.try_load_remote(name) {
      Ok(script) => {
        self.save_cache(name, &script).ok();
        Ok(script)
      }
      Err(_) => Err(err),
    }
  }

  fn try_load_cache(&self, name: &str) -> Option<String> {
    let file = self.cache_file_name(name);
    fs::read_to_string(file).ok()
  }

  fn save_cache(&self, name: &str, code: &str) -> rquickjs::Result<()> {
    Ok(fs::write(self.cache_file_name(name), code)?)
  }

  fn try_load_remote(&self, name: &str) -> rquickjs::Result<String> {
    let client = reqwest::blocking::ClientBuilder::new()
      .user_agent(crate::util::USER_AGENT)
      .build()
      .map_err(|_| rquickjs::Error::new_loading(name))?;

    let source = client
      .get(name)
      .send()
      .and_then(|r| r.error_for_status())
      .and_then(|r| r.text())
      .map_err(|_| rquickjs::Error::new_loading(name))?;

    Ok(source)
  }
}

impl Loader for RemoteLoader {
  fn load(&mut self, _ctx: &Ctx, name: &str) -> rquickjs::Result<ModuleData> {
    let err = rquickjs::Error::new_loading(name);
    if !self.name_valid(name) {
      return Err(err);
    };

    let source = self.try_load(name)?;
    Ok(ModuleData::source(name, source))
  }
}

fn uri_to_hash(uri: PathBuf) -> String {
  use std::hash::{DefaultHasher, Hash, Hasher};
  let mut hasher = DefaultHasher::new();
  uri.hash(&mut hasher);
  // length(u64 hex) == 16
  let hash = hasher.finish();
  // length == 30
  let filename = uri
    .to_string_lossy()
    .chars()
    .skip(5)
    .filter(|c| c.is_ascii_alphanumeric())
    .take(30)
    .collect::<String>();

  // final length is shorter than 64 bytes
  format!("{hash:x}{filename}")
}
