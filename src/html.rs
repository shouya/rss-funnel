use ego_tree::{NodeId, NodeRef};
use scraper::{Html, Selector};

const RELATIVE_URL_PROPERTIES: [(&str, &str); 3] = [
  ("*[href]", "href"),
  ("*[src]", "src"),
  ("*[srcset]", "srcset"),
];

pub fn convert_relative_url(html: &mut Html, base_url: &str) {
  use html5ever::{namespace_url, ns, LocalName, QualName};
  lazy_static::lazy_static! {
    static ref SELECTORS: Vec<(Selector, &'static str)> = {
      RELATIVE_URL_PROPERTIES
        .iter()
        .map(|(selector, attr)| (Selector::parse(selector).expect("bad selector"), *attr))
        .collect()
    };
  }

  let Ok(base_url) = url::Url::parse(base_url) else {
    return;
  };

  for (selector, attr) in SELECTORS.iter() {
    let node_ids = html.select(selector).map(|e| e.id()).collect::<Vec<_>>();
    for node_id in node_ids {
      let mut node = html.tree.get_mut(node_id).expect("unreachable");

      let scraper::Node::Element(elem) = node.value() else {
        continue;
      };

      let attr_name = QualName::new(None, ns!(), LocalName::from(*attr));
      let Some(attr_value) = elem.attrs.get_mut(&attr_name) else {
        continue;
      };

      let Ok(url) = base_url.join(attr_value) else {
        continue;
      };

      attr_value.clear();
      attr_value.push_slice(url.as_str());
    }
  }
}

pub fn html_body(html: &str) -> String {
  Html::parse_document(html)
    .select(&Selector::parse("body").unwrap())
    .next()
    .map(|body| body.inner_html().trim().to_string())
    .unwrap_or_else(|| html.to_string())
}

pub fn fragment_root_node_id(mut node: NodeRef<'_, scraper::Node>) -> NodeId {
  let val = node.value();
  if val.is_fragment() || val.is_document() {
    node = node.first_child().unwrap();
    return fragment_root_node_id(node);
  }

  if val.as_element().is_some_and(|e| e.name() == "html") {
    node = node.first_child().unwrap();
    return fragment_root_node_id(node);
  }

  node.id()
}

#[cfg(test)]
mod test {
  #[test]
  fn test_html_body() {
    let html = r#"
      <html>
        <head>
          <title>Test</title>
        </head>
        <body>
          <p>Test</p>
        </body>
      </html>
    "#;

    let expected = r#"<p>Test</p>"#;

    assert_eq!(super::html_body(html), expected);
  }
}
