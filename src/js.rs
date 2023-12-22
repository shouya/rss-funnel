mod builtin;

use std::fs;
use std::path::PathBuf;

use blake2s_simd::blake2s;
use rquickjs::loader::{
  BuiltinLoader, BuiltinResolver, FileResolver, Loader, Resolver, ScriptLoader,
};
use rquickjs::markers::ParallelSend;
use rquickjs::module::ModuleData;
use rquickjs::prelude::IntoArgs;
use rquickjs::{Context, Ctx, FromJs, Function, IntoJs, Module, Value};

use crate::util::{Error, Result};

pub struct Runtime {
  context: rquickjs::Context,
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

impl<'js, T> FromJs<'js> for AsJson<T>
where
  T: serde::de::DeserializeOwned,
{
  fn from_js(ctx: &Ctx<'js>, value: Value<'js>) -> rquickjs::Result<Self> {
    let json = ctx
      .json_stringify(value)?
      .and_then(|s| s.to_string().ok())
      .unwrap_or_else(|| "null".to_string());

    let value = serde_json::from_str(&json).unwrap();
    Ok(Self(value))
  }
}

impl Runtime {
  pub async fn new() -> Result<Self> {
    let runtime = rquickjs::Runtime::new()?;
    // limit memory usage to 32MB
    runtime.set_memory_limit(32 * 1024 * 1024);
    // limit max_stack_size to 1MB
    runtime.set_max_stack_size(1024 * 1024);

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
    runtime.set_loader(resolver, loader);

    let context = Context::full(&runtime)?;
    context.with(|ctx| builtin::register_builtin(&ctx))?;

    Ok(Self { context })
  }

  #[allow(unused)]
  pub async fn set_global<T>(&self, key: &str, value: T)
  where
    T: for<'js> IntoJs<'js> + ParallelSend,
  {
    self.context.with(|ctx| {
      let val = value.into_js(&ctx).unwrap();
      ctx.globals().set(key, val).unwrap();
    })
  }

  // return exported names
  pub async fn eval_module(
    &self,
    name: &str,
    code: &str,
  ) -> Result<Vec<String>> {
    let code = code.to_string();

    let mut names = Vec::new();
    self.context.with(|ctx: Ctx<'_>| -> Result<()> {
      let module = Module::evaluate(ctx.clone(), name, code);

      if let Err(rquickjs::Error::Exception) = module {
        let exception = ctx.catch();
        let exception_repr = format!("{:?}", exception.as_exception().unwrap());
        return Err(Error::JsException(exception_repr));
      }

      let globals = ctx.globals();

      for item in module?.entries() {
        let (name, value): (String, Value) = item?;
        globals.set(&name, value)?;
        names.push(name);
      }

      Ok(())
    })?;

    self.context.runtime().execute_pending_job().ok();
    Ok(names)
  }

  pub async fn eval<V>(&self, code: &str) -> Result<V>
  where
    V: for<'js> FromJs<'js> + ParallelSend,
  {
    let code = code.to_string();

    let res = self.context.with(|ctx: Ctx<'_>| -> Result<V> {
      let res = ctx.eval(code);

      if let Err(rquickjs::Error::Exception) = res {
        let exception = ctx.catch();
        let exception_repr = format!("{:?}", exception.as_exception().unwrap());
        return Err(Error::JsException(exception_repr));
      }

      Ok(res?)
    });

    self.context.runtime().execute_pending_job().ok();

    res
  }

  pub async fn fn_exists(&self, name: &str) -> bool {
    self.context.runtime().execute_pending_job().ok();
    self.context.with(|ctx| -> bool {
      let value: Result<Function<'_>, _> = ctx.globals().get(name);
      value.is_ok()
    })
  }

  pub async fn call_fn<V, Args>(&self, name: &str, args: Args) -> Result<V>
  where
    V: for<'js> FromJs<'js> + ParallelSend,
    Args: for<'js> IntoArgs<'js> + ParallelSend,
  {
    self.context.runtime().execute_pending_job().ok();

    self.context.with(|ctx| -> Result<V> {
      let value: Result<Function<'_>, _> = ctx.globals().get(name);
      let Ok(fun) = value else {
        return Err(Error::JsException(format!("function {} not found", name)));
      };

      let res = fun.call(args);

      if let Err(rquickjs::Error::Exception) = res {
        let exception = ctx.catch();
        let exception_repr = format!("{:?}", exception.as_exception().unwrap());
        return Err(Error::JsException(exception_repr));
      }

      Ok(res?)
    })
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

    if is_remote(base) && (name.starts_with("./") || name.starts_with("../")) {
      let mut path = PathBuf::from(base);
      path.pop();
      path.push(name.trim_start_matches("./"));
      return Ok(path.to_string_lossy().to_string());
    }

    Err(rquickjs::Error::new_loading(name))
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
    let digest = blake2s(uri.to_string_lossy().as_bytes()).to_hex();
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

  fn save_cache(&self, name: &str, code: &str) -> Result<()> {
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
