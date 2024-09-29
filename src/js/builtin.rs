use base64::prelude::{Engine as _, BASE64_STANDARD};
use rquickjs::{
  class::Trace,
  function::{Async, Func},
  Class, Ctx,
};

use super::dom::{Node, DOM};
use super::fetch::fetch;
use crate::Result;

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
    let ty = value.type_name();
    println!("[log] ({ty}) {}", string_repr(value)?);
    Ok(())
  }

  fn error(&self, value: rquickjs::Value<'_>) -> Result<(), rquickjs::Error> {
    let ty = value.type_name();
    println!("[error] ({ty}) {}", string_repr(value)?);
    Ok(())
  }
}

fn string_repr(value: rquickjs::Value<'_>) -> Result<String, rquickjs::Error> {
  let ctx = value.ctx();
  if let Some(json) =
    ctx.json_stringify_replacer_space(value.clone(), rquickjs::Undefined, 4)?
  {
    return Ok(json.to_string().unwrap());
  }

  if let Some(string) = value.into_string() {
    return Ok(string.to_string().unwrap());
  }

  Ok("unknown value".to_owned())
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

  fn decode_base64(base64: String) -> Option<String> {
    let bytes = BASE64_STANDARD.decode(base64).ok()?;
    String::from_utf8(bytes).ok()
  }

  fn encode_base64(bytes: String) -> String {
    BASE64_STANDARD.encode(bytes)
  }

  fn encode_url(url: String) -> String {
    urlencoding::encode(&url).to_string()
  }

  fn decode_url(url: String) -> Option<String> {
    urlencoding::decode(&url).ok().map(|s| s.to_string())
  }
}
