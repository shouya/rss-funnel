use ego_tree::{NodeId, NodeMut, NodeRef};
use html5ever::{LocalName, QualName, namespace_url, ns};
use rquickjs::{
  Class, Ctx, Error, Exception, JsLifetime, Object,
  class::{Trace, Tracer},
  convert::FromIteratorJs,
  prelude::This,
};
use scraper::ElementRef;

use crate::{Result, util::fragment_root_node_id};

#[rquickjs::class]
#[derive(Clone, JsLifetime)]
pub struct DOM {
  html: scraper::Html,
  fragment: bool,
}

impl<'js> Trace<'js> for DOM {
  fn trace<'a>(&self, _tracer: Tracer<'a, 'js>) {
    // no object is reached from this object
  }
}

#[rquickjs::methods]
impl DOM {
  #[qjs(static)]
  fn parse_fragment(html: String) -> Option<Self> {
    let html = scraper::Html::parse_fragment(&html);
    Some(DOM {
      html,
      fragment: true,
    })
  }

  #[qjs(static)]
  fn parse_document(html: String) -> Option<Self> {
    let html = scraper::Html::parse_document(&html);
    Some(DOM {
      html,
      fragment: false,
    })
  }

  #[qjs(constructor)]
  fn new(s: String) -> Option<Self> {
    Self::parse_document(s)
  }

  fn to_html(&self) -> String {
    if self.fragment {
      // do not include the outmost "<html>" tag for fragment
      self.html.root_element().inner_html()
    } else {
      self.html.html()
    }
  }

  fn select<'js>(
    this: This<Class<'js, Self>>,
    ctx: Ctx<'js>,
    selector: String,
  ) -> Result<Vec<Node<'js>>, Error> {
    let mut nodes = Vec::new();
    let selector = scraper::Selector::parse(&selector)
      .map_err(|_e| Exception::throw_message(&ctx, "bad selector"))?;
    let dom = this.clone();
    for node in this.borrow().html.select(&selector) {
      let node_id = node.id();
      nodes.push(Node {
        dom: dom.clone(),
        node_id,
      });
    }

    Ok(nodes)
  }
}

#[derive(JsLifetime)]
#[rquickjs::class]
pub struct Node<'js> {
  dom: Class<'js, DOM>,
  node_id: NodeId,
}

impl<'js> Trace<'js> for Node<'js> {
  fn trace<'a>(&self, tracer: Tracer<'a, 'js>) {
    tracer.mark(&self.dom);
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
    let value = elem
      .value()
      .attr(&name)
      .map(std::string::ToString::to_string);
    Ok(value)
  }

  fn set_attr(&self, name: String, value: String) -> Result<(), Error> {
    let mut dom = self.dom.borrow_mut();
    let mut node = self.node_mut(&mut dom)?;
    let scraper::Node::Element(elem) = node.value() else {
      return Err(Exception::throw_message(
        self.dom.ctx(),
        "node is not an element",
      ));
    };

    let attr_name =
      QualName::new(None, namespace_url!(""), LocalName::from(name));
    elem.attrs.insert(attr_name, value.into());
    Ok(())
  }

  fn unset_attr(&self, name: String) -> Result<(), Error> {
    let mut dom = self.dom.borrow_mut();
    let mut node = self.node_mut(&mut dom)?;
    let scraper::Node::Element(elem) = node.value() else {
      return Err(Exception::throw_message(
        self.dom.ctx(),
        "node is not an element",
      ));
    };

    let attr_name = QualName::new(None, ns!(), LocalName::from(name));
    elem.attrs.remove(&attr_name);
    Ok(())
  }

  fn tag_name(&self) -> Result<String, Error> {
    let dom = self.dom.borrow();
    let elem = self.elem(&dom)?;
    Ok(elem.value().name().to_string())
  }

  fn set_inner_text(&self, text: String) -> Result<(), Error> {
    use scraper::node::Text;
    let mut dom = self.dom.borrow_mut();
    let mut node = self.node_mut(&mut dom)?;
    while let Some(mut child) = node.first_child() {
      child.detach();
    }

    let new_text_node = scraper::Node::Text(Text { text: text.into() });
    node.append(new_text_node);
    Ok(())
  }

  fn delete(&self) -> Result<(), Error> {
    let mut dom = self.dom.borrow_mut();
    let mut node = self.node_mut(&mut dom)?;
    node.detach();
    Ok(())
  }

  fn inner_text(&self) -> Result<String, Error> {
    let dom = self.dom.borrow();
    let elem = self.elem(&dom)?;
    let text = elem.text().collect::<String>();
    Ok(text)
  }

  fn inner_html(&self) -> Result<String, Error> {
    let dom = self.dom.borrow();
    let elem = self.elem(&dom)?;
    let html = elem.inner_html();
    Ok(html)
  }

  fn set_inner_html(&self, html: String) -> Result<(), Error> {
    let mut dom = self.dom.borrow_mut();
    let mut node = self.node_mut(&mut dom)?;
    while let Some(mut child) = node.first_child() {
      child.detach();
    }

    let new_tree = scraper::Html::parse_fragment(&html).tree;
    let new_root = node.tree().extend_tree(new_tree);
    let root_node_id = fragment_root_node_id(new_root.into());
    node.append_id(root_node_id);
    Ok(())
  }

  fn outer_html(&self) -> Result<String, Error> {
    let dom = self.dom.borrow();
    let elem = self.elem(&dom)?;
    let html = elem.html();
    Ok(html)
  }

  fn set_outer_html(&self, html: String) -> Result<(), Error> {
    let mut dom = self.dom.borrow_mut();
    let mut node = self.node_mut(&mut dom)?;
    let new_tree = scraper::Html::parse_fragment(&html).tree;
    let new_root = node.tree().extend_tree(new_tree);
    let root_node_id = fragment_root_node_id(new_root.into());

    node.insert_id_after(root_node_id);
    node.detach();

    Ok(())
  }

  fn select(
    &self,
    ctx: Ctx<'js>,
    selector: String,
  ) -> Result<Vec<Node<'js>>, Error> {
    let selector = scraper::Selector::parse(&selector)
      .map_err(|_e| Exception::throw_message(&ctx, "bad selector"))?;

    let dom = self.dom.borrow();
    let elem = self.elem(&dom)?;

    let mut nodes = Vec::new();
    for node in elem.select(&selector) {
      let node_id = node.id();
      nodes.push(Node {
        dom: self.dom.clone(),
        node_id,
      });
    }

    Ok(nodes)
  }

  fn children(&self) -> Result<Vec<Node<'js>>, Error> {
    let dom = self.dom.borrow();
    let node = self.elem(&dom)?;
    let mut nodes = Vec::new();
    for child in node.children() {
      let node_id = child.id();
      nodes.push(Node {
        dom: self.dom.clone(),
        node_id,
      });
    }

    Ok(nodes)
  }

  fn previous_sibling(&self) -> Result<Option<Node<'js>>, Error> {
    let dom = self.dom.borrow();
    let node = self.node_ref(&dom)?;
    let prev = node.prev_sibling().map(|n| Node {
      dom: self.dom.clone(),
      node_id: n.id(),
    });
    Ok(prev)
  }

  fn next_sibling(&self) -> Result<Option<Node<'js>>, Error> {
    let dom = self.dom.borrow();
    let node = self.node_ref(&dom)?;
    let next = node.next_sibling().map(|n| Node {
      dom: self.dom.clone(),
      node_id: n.id(),
    });
    Ok(next)
  }

  fn parent(&self) -> Result<Option<Node<'js>>, Error> {
    let dom = self.dom.borrow();
    let node = self.node_ref(&dom)?;
    let parent = node.parent().map(|n| Node {
      dom: self.dom.clone(),
      node_id: n.id(),
    });
    Ok(parent)
  }

  fn node_type(&self) -> String {
    let dom = self.dom.borrow();
    let node = self.node_ref(&dom).unwrap();
    let val = node.value();
    match val {
      scraper::Node::Text(_) => "text".to_string(),
      scraper::Node::Element(_) => "element".to_string(),
      _ => "other".to_string(),
    }
  }

  fn remove(&self) {
    self.node_mut(&mut self.dom.borrow_mut()).unwrap().detach();
  }

  #[qjs(skip)]
  fn node_ref<'a, 'b: 'a>(
    &'a self,
    dom: &'b DOM,
  ) -> Result<NodeRef<'b, scraper::Node>, Error> {
    let node_ref = dom.html.tree.get(self.node_id).ok_or_else(|| {
      Exception::throw_message(self.dom.ctx(), "node not found")
    })?;

    Ok(node_ref)
  }

  #[qjs(skip)]
  fn node_mut<'a, 'b: 'a>(
    &'a self,
    dom: &'b mut DOM,
  ) -> Result<NodeMut<'b, scraper::Node>, Error> {
    let node_mut = dom.html.tree.get_mut(self.node_id).ok_or_else(|| {
      Exception::throw_message(self.dom.ctx(), "node not found")
    })?;

    Ok(node_mut)
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

#[cfg(test)]
mod test {
  use crate::js::Runtime;

  #[tokio::test]
  async fn test_dom_constructor() {
    let res = run_js(
      r#"
      const dom = new DOM("<div>hello</div>");
      dom.select('div').length.toString()
    "#,
    )
    .await;

    assert_eq!(res, "1");
  }

  #[tokio::test]
  async fn test_dom_constructor_document() {
    let res = run_js(
      r#"
      const dom = new DOM("<html><head></head><body><div>hello</div></body></html>");
      dom.to_html()
    "#,
    )
    .await;

    assert_eq!(
      res,
      "<html><head></head><body><div>hello</div></body></html>"
    );
  }

  #[tokio::test]
  async fn test_dom_to_html() {
    let res = run_js(
      r#"
      const dom = DOM.parse_fragment("<div>hello</div>");
      dom.to_html()
    "#,
    )
    .await;

    assert_eq!(res, "<div>hello</div>");
  }

  #[tokio::test]
  async fn test_node_attr() {
    let res = run_js(
      r#"
      const dom = new DOM("<div class='greeting' onclick='return 0;'>hello</div>");
      const [div] = dom.select('div');
      JSON.stringify(div.attrs())
    "#,
    )
    .await;

    let json = serde_json::from_str::<serde_json::Value>(&res).unwrap();
    assert_eq!(
      json,
      serde_json::json!({
        "class": "greeting",
        "onclick": "return 0;",
      })
    );
  }

  #[tokio::test]
  async fn test_node_mutation() {
    let res = run_js(
      r#"
      const dom = DOM.parse_fragment("<div><p>hello</p></div>");
      const [div] = dom.select('div');
      div.set_attr('class', 'greeting');
      const [p] = div.select('p');
      p.set_inner_text('world');
      dom.to_html()
      "#,
    );

    assert_eq!(res.await, "<div class=\"greeting\"><p>world</p></div>");
  }

  #[tokio::test]
  async fn test_node_deletion() {
    let res = run_js(
      r#"
      const dom = DOM.parse_fragment("<div><p>hello</p><p>world</p></div>");
      for (const p of dom.select('p')) {
        p.delete();
      }
      dom.to_html()
      "#,
    );

    assert_eq!(res.await, "<div></div>");
  }

  #[tokio::test]
  async fn test_set_inner_html() {
    let res = run_js(
      r#"
      const dom = DOM.parse_fragment("<div><p>hello</p></div>");
      dom.select('p')[0].set_inner_html("<span>world</span>");
      dom.to_html()
      "#,
    );

    assert_eq!(res.await, "<div><p><span>world</span></p></div>");
  }

  #[tokio::test]
  async fn test_set_outer_html() {
    let res = run_js(
      r#"
      const dom = DOM.parse_fragment("<div><p>hello</p></div>");
      dom.select('p')[0].set_outer_html("<span>world</span>");
      dom.to_html()
      "#,
    );

    assert_eq!(res.await, "<div><span>world</span></div>");
  }

  #[tokio::test]
  async fn test_set_inner_text() {
    let res = run_js(
      r#"
      const dom = DOM.parse_fragment("<div><p>hello</p></div>");
      dom.select('p')[0].set_inner_text("world");
      dom.to_html()
      "#,
    );

    assert_eq!(res.await, "<div><p>world</p></div>");
  }

  #[tokio::test]
  async fn test_node_children() {
    let res = run_js(
      r#"
      const dom = DOM.parse_fragment("<div><p>hello</p><p>world</p></div>");
      const [div] = dom.select('div');
      const children = div.children();
      children.map(c => c.tag_name()).join(',')
      "#,
    )
    .await;

    assert_eq!(res, "p,p");
  }

  #[tokio::test]
  async fn test_node_parent() {
    let res = run_js(
      r#"
      const dom = DOM.parse_fragment("<div><p>hello</p></div>");
      const [p] = dom.select('p');
      p.parent().tag_name()
      "#,
    )
    .await;

    assert_eq!(res, "div");
  }

  #[tokio::test]
  async fn test_node_siblings() {
    let res = run_js(
      r#"
      const dom = DOM.parse_fragment("<div>1</div><p>2</p><br><span>3</span>");
      const [p] = dom.select('p');
      const prev = p.previous_sibling().tag_name();
      const next = p.next_sibling().tag_name();
      `${prev},${next}`
      "#,
    )
    .await;

    assert_eq!(res, "div,br");
  }

  #[tokio::test]
  async fn test_node_remove() {
    let res = run_js(
      r#"
      const dom = DOM.parse_fragment("<div><p>hello</p></div>");
      const [p] = dom.select('p');
      p.remove();
      dom.to_html()
      "#,
    )
    .await;

    assert_eq!(res, "<div></div>");
  }

  async fn run_js(code: &str) -> String {
    let rt = Runtime::new().await.unwrap();
    rt.eval(code).await.unwrap()
  }
}
