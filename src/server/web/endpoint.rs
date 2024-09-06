use duration_str::HumanFormat as _;
use either::Either;
use maud::{html, Markup, PreEscaped, DOCTYPE};
use url::Url;

use crate::{
  client::ClientConfig,
  feed::{Feed, PostPreview},
  filter::FilterConfig,
  filter_pipeline::FilterPipelineConfig,
  server::{endpoint::EndpointService, web::sprite, EndpointParam},
  source::{FromScratch, Source},
};

pub async fn render_endpoint_page(
  endpoint: EndpointService,
  path: String,
  param: EndpointParam,
) -> Markup {
  // render source control
  let source = source_control_fragment(&path, endpoint.source(), &param);

  // render config
  let config = render_config_fragment(&endpoint);

  // render config
  let filters =
    render_topmost_filter_pipeline_fragment(&endpoint.config().filters);

  // render feed preview
  let feed = fetch_and_render_feed(endpoint, param).await;

  html! {
    (DOCTYPE)
    head {
      title { "RSS Funnel" }
      meta charset="utf-8";
      (super::header_libs_fragment());
      script { (PreEscaped(inline_script())) }
      style { (PreEscaped(inline_styles())) }
    }
    body {
      main {
        span style="float:left; margin-right: 2rem;" { a href="/_/" { "Back" } }
        h2 { (path) }

        div {
          @if let Some(source) = source {
            (source)
          }

          details open="" {
            summary { "Configuration" }
            section .config-section {
              (config)
            }
          }

          details .filters-section {
            summary { "Filters" }
            (filters)
          }
        }

        section .feed-section {
          (feed)
        }
      }
    }
  }
}

fn source_control_fragment(
  path: &str,
  source: &Option<Source>,
  param: &EndpointParam,
) -> Option<Markup> {
  match source {
    None => Some(html! {
      div style="display: flex; position: relative;" {
        input
          style="flex-grow: 1;"
          type="text"
          name="source"
          placeholder="Source URL"
          value=[param.source().map(|url| url.as_str())]
          hx-get=(format!("/_/endpoint/{path}"))
          hx-trigger="keyup changed delay:500ms"
          hx-push-url="true"
          hx-indicator=".loading"
          hx-target="body"
        {}
        div.loading { (sprite("loader")) }
      }
    }),
    Some(Source::AbsoluteUrl(url)) => Some(html! {
      div .source.absolute { (url) }
    }),
    Some(Source::RelativeUrl(url)) => Some(html! {
      div .source.relative { (url) }
    }),
    Some(Source::Templated(templated)) => {
      Some(source_template_fragment(templated))
    }
    Some(Source::FromScratch(scratch)) => Some(from_scratch_fragment(scratch)),
  }
}

fn from_scratch_fragment(scratch: &FromScratch) -> Markup {
  html! {
    table {
      tbody {
        tr {
          th { "Format" }
          td { (scratch.format.as_str()) }
        }
        tr {
          th { "Title" }
          td { (scratch.title) }
        }
        @if let Some(link) = &scratch.link {
          tr {
            th { "Link" }
            td { (link) }
          }
        }
        @if let Some(description) = &scratch.description {
          tr {
            th { "Description" }
            td { (description) }
          }
        }
      }
    }
  }
}

fn source_template_fragment(templated: &crate::source::Templated) -> Markup {
  html! {
    @for fragment in templated.fragments() {
      @match fragment {
        Either::Left(plain) => span { (plain) },
        Either::Right((name, Some(placeholder))) => {
          @let value=placeholder.default.as_ref();
          @let validation=placeholder.validation.as_ref();
          input
            id={"placeholder-" (name)}
            name=(name)
            placeholder=(name)
            pattern=[validation]
            value=[value]
          {}
        }
        Either::Right((name, None)) => {
          span style="color: red" title="Placeholder not defined" { "${" (name) "}" }
        }
      }
    }
  }
}

fn render_config_fragment(endpoint: &EndpointService) -> Markup {
  let config = endpoint.config();

  html! {
    table {
      tr {
        th { "OTF filters" }
        td {
          @if config.on_the_fly_filters {
            "Enabled"
          } @else {
            "Disabled"
          }
        }
      }

      @if let Some(client) = &config.client {
        tr {
          th { "Client" }
          td { (client_fragment(client)) }
        }
      }
    }
  }
}

fn client_fragment(client: &ClientConfig) -> Markup {
  html! {
    table {
      @if let Some(user_agent) = &client.user_agent {
        tr {
          th { "User-Agent" }
          td { (user_agent) }
        }
      }

      @if let Some(accept) = &client.accept {
        tr {
          th { "Accept" }
          td { (accept) }
        }
      }

      @if let Some(cookie) = (client.cookie.as_ref()).or(client.set_cookie.as_ref()) {
        tr {
          th { "Cookie" }
          td { (cookie) }
        }
      }

      @if let Some(referer) = &client.referer {
        tr {
          th { "Referer" }
          td { (referer) }
        }
      }

      @if client.accept_invalid_certs {
        tr {
          th { "Accept invalid certs" }
          td { "Yes" }
        }
      }

      @if let Some(proxy) = &client.proxy {
        tr {
          th { "Proxy" }
          td { (proxy) }
        }
      }

      @if let Some(timeout) = &client.timeout {
        tr {
          th { "Timeout" }
          td { (timeout.human_format()) }
        }
      }

      @if let Some(cache_size) = &client.cache_size {
        tr {
          th { "Cache size" }
          td { (cache_size) }
        }
      }

      @if let Some(cache_ttl) = &client.cache_ttl {
        tr {
          th { "Cache TTL" }
          td { (cache_ttl.human_format()) }
        }
      }

      @if let Some(assume_content_type) = &client.assume_content_type {
        tr {
          th { "Assume content type" }
          td { (assume_content_type) }
        }
      }
    }
  }
}

fn render_topmost_filter_pipeline_fragment(
  filters: &FilterPipelineConfig,
) -> Markup {
  render_filter_pipeline_fragment(filters, true, true)
}

fn render_nested_filter_pipeline_fragment(
  filters: &FilterPipelineConfig,
) -> Markup {
  render_filter_pipeline_fragment(filters, false, false)
}

fn render_filter_pipeline_fragment(
  filters: &FilterPipelineConfig,
  _render_index: bool,
  _render_header: bool,
) -> Markup {
  html! {
    nav {
      ul {
        @for filter in &filters.filters {
          var { (filter.name()) }
          li {
            @match filter {
              FilterConfig::Js(conf) => (conf),
              FilterConfig::ModifyPost(conf) => (conf),
              FilterConfig::ModifyFeed(conf) => (conf),
              FilterConfig::FullText(_conf) => ("TODO: render filter"),
              FilterConfig::SimplifyHtml(_conf) => ("TODO: render filter"),
              FilterConfig::RemoveElement(_conf) => ("TODO: render filter"),
              FilterConfig::KeepElement(_conf) => ("TODO: render filter"),
              FilterConfig::Split(_conf) => ("TODO: render filter"),
              FilterConfig::Sanitize(_conf) => ("TODO: render filter"),
              FilterConfig::KeepOnly(_conf) => ("TODO: render filter"),
              FilterConfig::Discard(_conf) => ("TODO: render filter"),
              FilterConfig::Highlight(_conf) => ("TODO: render filter"),
              FilterConfig::Merge(conf) => (conf),
              FilterConfig::Note(_conf) => ("TODO: render filter"),
              FilterConfig::ConvertTo(_conf) => ("TODO: render filter"),
              FilterConfig::Limit(_conf) => ("TODO: render filter"),
              FilterConfig::Magnet(_conf) => ("TODO: render filter"),
              FilterConfig::ImageProxy(_conf) => ("TODO: render filter"),
            }
          }
        }
      }
    }
  }
}

async fn fetch_and_render_feed(
  endpoint: EndpointService,
  params: EndpointParam,
) -> Markup {
  html! {
    @match endpoint.run(params).await {
      Ok(feed) => (render_feed(&feed)),
      Err(e) => {
        p { "Failed to fetch feed:" }
        p { (e.to_string()) }
      }
    }
  }
}

fn render_post(post: PostPreview) -> Markup {
  let link_url = Url::parse(&post.link).ok();

  html! {
    article data-display-mode="rendered" data-folded="true" .post-entry {
      header .flex {
        span .icon-container.fold-icon onclick="toggleFold(this)" title="Toggle fold" {
          (sprite("caret-right"))
        }
        span .icon-container.raw-icon  onclick="toggleRaw(this)" title="Toggle HTML" {
          (sprite("source-code"))
        }

        div .row.flex.grow style="margin-left: .5rem" { (post.title); (external_link(&post.link)) }
      }
      @if let Some(body) = &post.body {
        section {
          div .entry-content.rendered {
            template shadowrootmode="open" {
              (PreEscaped(santize_html(body, link_url)))
            }
          }
          div .entry-content.raw {
            pre { (body) }
          }
        }

      } @else {
        section { "No body" }
      }
      footer {
        @if let Some(date) = post.date {
          time .inline datetime=(date.to_rfc3339()) { (date.to_rfc2822()) }
        }
        @if let Some(author) = &post.author {
          span .ml-1 {
            ("By");
            address .inline rel="author" { (author) }
          }
        }
      }
    }
  }
}

fn render_feed(feed: &Feed) -> Markup {
  let preview = feed.preview();

  html! {
    h3 style="display:flex" {
      (preview.title);
      (external_link(&preview.link))
    }
    @if let Some(description) = &preview.description {
      p { (description) }
    }
    p style="clear:both" { (format!("Entries ({}):", preview.posts.len())) }

    @for post in preview.posts {
      (render_post(post))
    }
  }
}

fn inline_styles() -> &'static str {
  r#"
  .icon {
    transition: all 0.2s;
  }

  .post-entry {
    margin-left: 0 !important;
    margin-right: 0 !important;

    .icon-container {
      display: inline-flex;
      align-self: center;
    }

    &[data-folded="false"] {
      .fold-icon > .icon {
        transform: rotate(90deg);
      }
    }
    &[data-folded="true"] {
      header {
        border: 0 !important;
        margin-bottom: 0 !important;
        padding-bottom: 0 !important;
      }

      section, footer {
        display: none;
      }
    }

    .entry-content {
      display: none;
    }
    &[data-display-mode="rendered"] {
      .entry-content.rendered {
        display: block;
      }
    }
    &[data-display-mode="raw"] {
      .raw-icon > .icon {
        color: var(--active);
      }
      .entry-content.raw {
        display: block;
      }
    }
  }

  .source {
    font-family: monospace;
  }

  @keyframes rotation {
    from { transform: rotate(0deg); }
    to   { transform: rotate(360deg); }
  }

  .loading {
    display: none;
    position: absolute;
    right: 0.5rem;
    align-items: center;
    height: 100%;

    &.htmx-request {
      display: flex;
      animation: rotation 2s infinite linear;
    }
  }
  "#
}

fn inline_script() -> &'static str {
  r#"
  function toggleFold(element) {
    const article = element.closest("article");
    article.dataset.folded = article.dataset.folded === "false";
  }

  function toggleRaw(element) {
    const article = element.closest("article");
    article.dataset.displayMode =
      article.dataset.displayMode === "rendered" ? "raw" : "rendered";
  }
  "#
}

// requires the container to have a `display: flex` style
fn external_link(url: &str) -> Markup {
  html! {
    a style="margin-left: 0.25rem;display:inline-flex;align-self:center"
      href=(url) {
      (sprite("external-link"))
    }
  }
}

fn santize_html(html: &str, base: Option<Url>) -> String {
  use ammonia::UrlRelative;
  let mut builder = ammonia::Builder::new();
  if let Some(base) = base {
    builder.url_relative(UrlRelative::RewriteWithBase(base));
  }
  builder.clean(html).to_string()
}
