use std::collections::HashMap;

use either::Either;
use maud::{html, Markup, PreEscaped, DOCTYPE};
use url::Url;

use crate::{
  feed::{Feed, PostPreview},
  server::{endpoint::EndpointService, web::sprite, EndpointParam},
  source::{FromScratch, Source},
};

pub async fn render_endpoint_page(
  endpoint: EndpointService,
  path: String,
  param: Result<EndpointParam, String>,
) -> Markup {
  // render source control
  let source = source_control_fragment(&path, endpoint.source(), &param);

  // render config
  let config = render_config_fragment(&path, param.as_ref().ok(), &endpoint);

  // render feed preview
  let feed = match param {
    Ok(param) => fetch_and_render_feed(endpoint, param).await,
    Err(e) => html! {
      div .flash.danger {
        header { b { "Invalid request params" } }
        p { (e) }
      }
    },
  };

  html! {
    (DOCTYPE)
    head {
      title { "RSS Funnel" }
      meta charset="utf-8";
      (super::favicon());
      (super::header_libs_fragment());
      script { (PreEscaped(inline_script())) }
      style { (PreEscaped(inline_styles())) }
      link rel="stylesheet"
        referrerpolicy="no-referrer"
        href="https://unpkg.com/prismjs@v1.x/themes/prism.css";
      script
        src="https://unpkg.com/prismjs@v1.x/components/prism-core.min.js"
        referrerpolicy="no-referrer" {}
      script
        src="https://unpkg.com/prismjs@v1.x/plugins/autoloader/prism-autoloader.min.js"
        referrerpolicy="no-referrer" {}
    }
    body {
      header .header-bar {
        button .back-button {
          a href="/_/" { "Back" }
        }
        h2 { (path) }
        button .copy-button title="Copy Endpoint URL" onclick="copyToClipboard()" {
          (sprite("copy"))
        }
      }

      section {
        @if let Some(source) = source {
          section .source-control {
            (source);
            div.loading { (sprite("loader")) }
          }
        }

        details {
          summary { "Config" }
          section .config-section {
            (config)
          }
        }
      }

      main .feed-section {
        (feed)
      }
    }
  }
}

fn source_control_fragment(
  path: &str,
  source: &Option<Source>,
  param: &Result<EndpointParam, String>,
) -> Option<Markup> {
  match source {
    None => Some(html! {
      input
        .hx-included
        style="flex-grow: 1;"
        type="text"
        name="source"
        placeholder="Source URL"
        value=[param.as_ref().ok().and_then(|p| p.source()).map(|url| url.as_str())]
        hx-get=(format!("/_/endpoint/{path}"))
        hx-trigger="keyup changed delay:500ms"
        hx-push-url="true"
        hx-indicator=".loading"
        hx-include=".hx-included"
        hx-target="main"
        hx-select="main"
      {}
    }),
    Some(Source::AbsoluteUrl(url)) => Some(html! {
      div title="Source" .source { (url) }
    }),
    Some(Source::RelativeUrl(url)) => Some(html! {
      div title="Source" .source { (url) }
    }),
    Some(Source::Templated(templated)) => Some(html! {
      div .template-container {
        @let queries = param.as_ref().ok().map(|p| p.extra_queries());
        (source_template_fragment(templated, path, queries));
      }
    }),
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

fn source_template_fragment(
  templated: &crate::source::Templated,
  path: &str,
  queries: Option<&HashMap<String, String>>,
) -> Markup {
  html! {
    @for fragment in templated.fragments() {
      @match fragment {
        Either::Left(plain) => span style="white-space: nowrap" { (plain) },
        Either::Right((name, Some(placeholder))) => {
          @let value=queries.and_then(|q| q.get(name));
          @let default_value=placeholder.default.as_ref();
          @let value=value.or(default_value);
          @let validation=placeholder.validation.as_ref();
          input
            .source-template-placeholder.hx-included
            name=(name)
            placeholder=(name)
            pattern=[validation]
            value=[value]
            hx-get=(format!("/_/endpoint/{path}"))
            hx-trigger="keyup changed delay:500ms"
            hx-push-url="true"
            hx-include=".hx-included"
            hx-indicator=".loading"
            hx-target="main"
            hx-select="main"
            id={"placeholder-" (name)}
          {}
        }
        Either::Right((name, None)) => {
          span style="color: red" title="Placeholder not defined" { "${" (name) "}" }
        }
      }
    }
  }
}

fn render_config_fragment(
  path: &str,
  param: Option<&EndpointParam>,
  endpoint: &EndpointService,
) -> Markup {
  let config = endpoint.config();
  let filter_enabled = |i| {
    if let Some(f) = param.and_then(|p| p.filter_skip()) {
      f.allows_filter(i) as u8
    } else {
      true as u8
    }
  };

  html! {
    @if config.on_the_fly_filters {
      section {
        var .bg-variant.bd-variant.variant title="On-the-fly filters enabled" { "OTF" }
      }
    }

    @if let Some(client) = &config.client {
      section {
        header { b { "Custom client configuration:" } }
        @if let Ok(yaml) = client.to_yaml() {
          div .client-config {
            pre { code .language-yaml { (yaml) } }
          }
        }
      }
    }

    @let filters = &config.filters;
    @if filters.filters.is_empty() {
      "No filters"
    } @else {
      div {
        header { b { "Filters:" } }
        ul #filter-list .hx-included
          hx-vals="js:...gatherFilterSkip()"
          hx-get=(format!("/_/endpoint/{path}"))
          hx-trigger="click from:.filter-name"
          hx-push-url="true"
          hx-include=".hx-included"
          hx-indicator=".loading"
          hx-target="main"
          hx-select="main"
        {
          @for (i, filter) in filters.filters.iter().enumerate() {
            li .filter-item {
              var .filter-name title="Toggle"
                data-enabled=(filter_enabled(i))
                onclick="this.dataset.enabled=1-+this.dataset.enabled"
                data-index=(i) {
                  (filter.name())
                }

              @if let Ok(yaml) = filter.to_yaml() {
                // TODO: show help button
                div .filter-link {}
                div .filter-definition {
                  pre { code .language-yaml { (yaml) } }
                }
              }
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
        div .flash.danger {
          header { b { "Failed to fetch feed" } }
          p { (e.to_string()) }
        }
      }
    }
  }
}

fn render_post(post: PostPreview) -> Markup {
  let link_url = Url::parse(&post.link).ok();

  html! {
    article data-display-mode="rendered" data-folded="true" .post-entry {
      header .flex {
        span .icon-container.fold-icon onclick="toggleFold(this)" title="Expand" {
          (sprite("caret-right"))
        }
        span .icon-container.raw-icon  onclick="toggleRaw(this)" title="Raw HTML body" {
          (sprite("source-code"))
        }

        div .row.flex.grow style="margin-left: .5rem" { (post.title); (external_link(&post.link)) }
      }

      @if let Some(body) = &post.body {
        section {
          @let id = format!("entry-{}", rand_id());
          @let content = santize_html(body, link_url);
          div id=(id) .entry-content.rendered {
            template {
              style { (PreEscaped("max-width: 100%;")) }
              (PreEscaped(content))
            }
            script { (PreEscaped(format!("fillEntryContent('{id}')"))) }
          }
          div .entry-content.raw {
            pre { code .language-html { (body) } }
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
            (PreEscaped("By&nbsp;"));
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
    p { (format!("Entries ({}):", preview.posts.len())) }

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
        overflow-x: scroll;
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
    right: 1rem;
    align-items: center;
    height: 100%;

    &.htmx-request {
      display: flex;
      animation: rotation 2s infinite linear;
    }
  }

  .filter-item {
    position: relative;

    > var {
      cursor: pointer;
    }

    > .filter-definition, .filter-link {
      display: none;
    }

    &:hover > .filter-link {
      display: inline-block;
      border-top: 1px solid var(--bd-muted);
      border-bottom: 1px solid var(--bd-muted);
      margin-left: 0.2rem;
      width: 15rem;
      vertical-align: middle;
    }

    &:hover > .filter-definition {
      display: block;
      position: absolute;
      left: 15rem;
      top: 0;
      z-index: 1;
      border: 1px solid var(--bd-muted);
      border-radius: var(--bd-radius);
      box-shadow: 1px 2px 3px var(--bd-muted);
    }
  }

  .source-control {
    background-color: var(--bg-muted);
    padding: 1rem;
    border-radius: var(--bd-radius);
    display :flex;
    position: relative;
    align-items: center;
  }
  .source-template-container {
    display: flex;
    position: relative;
    align-items: baseline;
    flex-wrap: wrap;

    .source-template-placeholder {
      width: auto;
      display: inline-block;
    }
  }


  main.feed-section {
    background-color: var(--bg-muted);
    padding: 1.5rem;
    border-radius: var(--bd-radius);
  }

  .header-bar {
    margin: 1rem 0 !important;
    padding-bottom: 1rem;
    border-bottom: 1.5px dotted;
    display: flex;
    align-items: center;
    height: 2rem;

    > h2 {
      flex: 1;
      margin-bottom: 0;
    }

    > .back-button {
      margin-right: 2rem;

      a:hover {
        color: var(--bg-accent);
      }
    }

    > .copy-button {
      > svg {
        vertical-align: middle;
      }
    }
  }

  .filter-item {
    var.filter-name[data-enabled="0"] {
      color: var(--muted);
      background-color: var(--bg-muted);
    }
  }

  pre[class*="language"] {
    margin: 0 !important;
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

  function gatherFilterSkip() {
    const skipped = [...document.querySelectorAll(".filter-item > var")].filter(x=>!+x.dataset.enabled).map(x => x.dataset.index).join(",");
    if (skipped === "") {
      return {};
    } else {
      return {filter_skip: skipped};
    }
  }

  function fillEntryContent(id) {
    const parent = document.getElementById(id);
    const shadowRoot = parent.attachShadow({mode: 'open'});
    const content = parent.querySelector("template").innerHTML;
    parent.innerHTML = "";
    shadowRoot.innerHTML = content;
  }

  function copyToClipboard() {
    const url = window.location.href.replace(/\/_\/endpoint\//, "/");
    navigator.clipboard.writeText(url);
    alert("Copied: " + url);
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

fn rand_id() -> String {
  use rand::Rng as _;
  rand::thread_rng().gen::<u64>().to_string()
}
