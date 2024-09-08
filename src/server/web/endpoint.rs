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
  let config = render_config_fragment(&endpoint);

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
      (super::header_libs_fragment());
      script { (PreEscaped(inline_script())) }
      style { (PreEscaped(inline_styles())) }
    }
    body {
      header .header-bar {
        button .back-button {
          a href="/_/" { "Back" }
        }
        h2 { (path) }
      }

      section {
        @if let Some(source) = source {
          section .source-control {
            (source)
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
      div style="display: flex; position: relative;" {
        input
          style="flex-grow: 1;"
          type="text"
          name="source"
          placeholder="Source URL"
          value=[param.as_ref().ok().and_then(|p| p.source()).map(|url| url.as_str())]
          hx-get=(format!("/_/endpoint/{path}"))
          hx-trigger="keyup changed delay:500ms"
          hx-push-url="true"
          hx-indicator=".loading"
          hx-target="main"
          hx-select="main"
        {}
        div.loading { (sprite("loader")) }
      }
    }),
    Some(Source::AbsoluteUrl(url)) => Some(html! {
      div title="Source" .source { (url) }
    }),
    Some(Source::RelativeUrl(url)) => Some(html! {
      div title="Source" .source { (url) }
    }),
    Some(Source::Templated(templated)) => Some(html! {
      div style="display: flex; position: relative; align-items: baseline;" {
        @let queries = param.as_ref().ok().map(|p| p.extra_queries());
        (source_template_fragment(templated, path, queries));
        div.loading { (sprite("loader")) }
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
            .source-template-placeholder
            id={"placeholder-" (name)}
          name=(name)
            placeholder=(name)
            pattern=[validation]
            value=[value]
            hx-get=(format!("/_/endpoint/{path}"))
            hx-trigger="keyup changed delay:500ms"
            hx-push-url="true"
            hx-include=".source-template-placeholder"
            hx-indicator=".loading"
            hx-target="main"
            hx-select="main"
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
    @if config.on_the_fly_filters {
      section {
        var .bg-variant.bd-variant.variant { "On-the-fly filters enabled" }
      }
    }

    @if let Some(client) = &config.client {
      section {
        header { b { "Custom client configuration:" } }
        @if let Ok(yaml) = client.to_yaml() {
          div .client-config {
            pre { (yaml) }
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
        ul {
          @for filter in &filters.filters {
            li .filter-item {
              // TODO: support toggling individual filters
              var .filter-name title="Toggle" { (filter.name()) }
              @if let Ok(yaml) = filter.to_yaml() {
                // TODO: show help button
                div .filter-link {}
                div .filter-definition {
                  pre { (yaml) }
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
          div .entry-content.rendered style="overflow-x: scroll" {
            template shadowrootmode="open" {
              style {
                (PreEscaped("* { max-width: 100%; }"))
              }
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
    background-color: var(--bg-active);
    padding: 1rem;
    border-radius: var(--bd-radius);
  }
  .source-template-placeholder {
    width: auto;
    display: inline-block;
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

    > button {
      float:left;
      margin-right: 2rem;

      a:hover {
        color: var(--bg-accent);
      }
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
