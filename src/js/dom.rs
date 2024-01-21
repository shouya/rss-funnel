use ego_tree::NodeId;
use rquickjs::{
  class::{Trace, Tracer},
  convert::FromIteratorJs,
  Class, Ctx, Error, Exception, Object,
};
use scraper::ElementRef;

use crate::util::Result;

#[rquickjs::class]
#[derive(Clone)]
pub struct DOM {
  html: scraper::Html,
}

impl<'js> Trace<'js> for DOM {
  fn trace<'a>(&self, _tracer: Tracer<'a, 'js>) {
    // no object is reached from this object
  }
}

#[rquickjs::methods]
impl DOM {
  #[qjs(constructor)]
  fn new(s: String) -> Option<Self> {
    let html = scraper::Html::parse_document(&s);
    Some(DOM { html })
  }

  fn to_html(&self) -> String {
    self.html.html()
  }

  fn select<'js>(
    &self,
    ctx: Ctx<'js>,
    selector: String,
  ) -> Result<Vec<Node<'js>>, Error> {
    let mut nodes = Vec::new();
    let selector = scraper::Selector::parse(&selector)
      .map_err(|_e| Exception::throw_message(&ctx, "bad selector"))?;
    let dom = Class::instance(ctx, self.clone())?;
    for node in self.html.select(&selector) {
      let node_id = node.id();
      nodes.push(Node {
        dom: dom.clone(),
        node_id,
      });
    }

    Ok(nodes)
  }
}

#[rquickjs::class]
pub struct Node<'js> {
  dom: Class<'js, DOM>,
  node_id: NodeId,
}

impl<'js> Trace<'js> for Node<'js> {
  fn trace<'a>(&self, tracer: Tracer<'a, 'js>) {
    tracer.mark(&self.dom)
  }
}

#[rquickjs::methods]
impl<'js> Node<'js> {
  fn attrs(&self, ctx: Ctx<'js>) -> Result<Object<'js>, Error> {
    let dom = self.dom.borrow();
    let elem = self.elem(&dom)?;
    let kvs = elem.value().attrs();
    let obj = Object::from_iter_js(&ctx, kvs)?;
    Ok(obj)
  }

  fn attr(&self, name: String) -> Result<Option<String>, Error> {
    let dom = self.dom.borrow();
    let elem = self.elem(&dom)?;
    let value = elem.value().attr(&name).map(|s| s.to_string());
    Ok(value)
  }

  #[qjs(skip)]
  fn elem<'a, 'b: 'a>(
    &'a self,
    dom: &'b DOM,
  ) -> Result<scraper::ElementRef<'b>, Error> {
    let node_ref = dom.html.tree.get(self.node_id).ok_or_else(|| {
      Exception::throw_message(self.dom.ctx(), "node not found")
    })?;

    let elem_ref = ElementRef::wrap(node_ref).ok_or_else(|| {
      Exception::throw_message(self.dom.ctx(), "node is not an element")
    })?;

    Ok(elem_ref)
  }
}
