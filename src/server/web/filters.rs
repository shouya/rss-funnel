use maud::{html, Markup, Render};

use crate::{filter, source::SimpleSourceConfig, util::SingleOrVec};

impl Render for filter::js::JsConfig {
  fn render(&self) -> Markup {
    html! { pre {code { (self.code) }}}
  }
}

impl Render for filter::js::ModifyFeedConfig {
  fn render(&self) -> Markup {
    html! { pre {code { (self.code) }}}
  }
}

impl Render for filter::js::ModifyPostConfig {
  fn render(&self) -> Markup {
    html! { pre {code { (self.code) }}}
  }
}

impl Render for filter::merge::MergeConfig {
  fn render(&self) -> Markup {
    match self {
      filter::merge::MergeConfig::Simple(simple) => {
        render_single_or_vec(&simple.source, simple_source_url)
      }
      filter::merge::MergeConfig::Full(full) => {
        html! {
          table {
            tr {
              th { "Sources" }
              td { (render_single_or_vec(&full.source, simple_source_url)) }
            }
            @if let Some(parallelism) = full.parallelism {
              tr {
                th { "Parallelism" }
                td { (parallelism) }
              }
            }
            @if let Some(client) = &full.client {
              tr {
                th { "Client" }
                td { (collapsed("Client config", super::client_fragment(client))) }
              }
            }
            @if let Some(filters) = &full.filters {
              tr {
                th { "Filters" }
                @let filters = super::render_nested_filter_pipeline_fragment(filters);
                td { (collapsed("Filter pipeline", filters)) }
              }
            }
          }
        }
      }
    }
  }
}

fn simple_source_url(source: &SimpleSourceConfig) -> Markup {
  html! { a .source href=(source.0) { (source.0) } }
}

fn render_single_or_vec<T>(
  single_or_vec: &SingleOrVec<T>,
  mut f: impl FnMut(&T) -> Markup,
) -> Markup {
  match single_or_vec {
    SingleOrVec::Single(single) => f(single),
    SingleOrVec::Vec(vec) => html! {
      ul {
        @for item in vec {
          li { (f(item)) }
        }
      }
    },
  }
}

fn collapsed(title: &str, content: Markup) -> Markup {
  html! {
    details {
      summary { (title) }
      section { (content) }
    }
  }
}
