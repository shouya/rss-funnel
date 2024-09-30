use std::{borrow::Cow, collections::HashMap};

use either::Either;
use maud::{html, Markup, PreEscaped, DOCTYPE};
use url::Url;

use crate::{
  feed::{Feed, NormalizedPost, Post},
  filter::FilterContext,
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
  let config = render_config_fragment(&path, &param, &endpoint);
  let config_tags = render_config_header_tags(&endpoint);

  // render normalized feed
  let feed = fetch_and_render_feed(endpoint, param).await;

  html! {
    (DOCTYPE)
    head {
      title { "RSS Funnel" }
      meta charset="utf-8";
      (super::favicon());
      (super::header_libs_fragment());
      script { (PreEscaped(inline_script())) }
      style { (PreEscaped(super::common_styles())) }
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

      section .source-and-config {
        section .source-control {
          (source);
          div.loading { (sprite("loader")) }
        }

        details {
          summary {
            "Config";
            (config_tags)
          }
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
  source: &Source,
  param: &EndpointParam,
) -> Markup {
  match source {
    Source::Dynamic => html! {
      input
        .hx-included.grow
        type="text"
        name="source"
        placeholder="Source URL"
        value=[param.source().map(|url| url.as_str())]
        hx-get=(format!("/_/endpoint/{path}"))
        hx-trigger="keyup changed delay:500ms"
        hx-push-url="true"
        hx-indicator=".loading"
        hx-include=".hx-included"
        hx-target="main"
        hx-select="main"
      {}
    },
    Source::AbsoluteUrl(url) => html! {div title="Source" .source { (url) }},
    Source::RelativeUrl(url) => html! {div title="Source" .source { (url) }},
    Source::Templated(templated) => html! {
      div .source-template-container {
        @let queries = param.extra_queries();
        (source_template_fragment(templated, path, queries));
      }
    },
    Source::FromScratch(scratch) => from_scratch_fragment(scratch),
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
  queries: &HashMap<String, String>,
) -> Markup {
  html! {
    @for fragment in templated.fragments() {
      @match fragment {
        Either::Left(plain) => span style="white-space: nowrap" { (plain) },
        Either::Right((name, Some(placeholder))) => {
          @let value=queries.get(name);
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

fn render_config_header_tags(endpoint: &EndpointService) -> Markup {
  let config = endpoint.config();

  html! {
    @if config.on_the_fly_filters {
      div .tag-container {
        span .tag.otf title="On-the-fly filters enabled" { "OTF" }
      }
    }
  }
}

fn render_config_fragment(
  path: &str,
  param: &EndpointParam,
  endpoint: &EndpointService,
) -> Markup {
  let config = endpoint.config();
  let filter_enabled = |i| {
    if let Some(f) = param.filter_skip() {
      f.allows_filter(i) as u8
    } else {
      true as u8
    }
  };

  html! {
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
          hx-vals="js:{...gatherFilterSkip()}"
          hx-get=(format!("/_/endpoint/{path}"))
          hx-trigger="click from:.filter-name"
          hx-push-url="true"
          hx-include=".hx-included"
          hx-indicator=".loading"
          hx-target="main"
          hx-select="main"
        {
          @for (i, filter) in filters.filters.iter().enumerate() {
            li {
              div .filter-item.flex.flex-center {
                span .filter-name title="Toggle"
                  data-enabled=(filter_enabled(i))
                  onclick="this.dataset.enabled=1-+this.dataset.enabled"
                  data-index=(i) {
                    (filter.name())
                  }

                (filter_doc(filter.name()));

                @if let Ok(yaml) = filter.to_yaml() {
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
}

async fn fetch_and_render_feed(
  endpoint: EndpointService,
  params: EndpointParam,
) -> Markup {
  let mut context = FilterContext::from_param(&params);
  context.enable_logging();

  html! {
    @match endpoint.run_with_context(&mut context, params).await {
      Ok(feed) => (render_feed(feed, context.logs())),
      Err(e) => {
        div .flash.error {
          header { b { "Failed to fetch feed" } }
          p { (e.to_string()) }
        }
      }
    }
  }
}

fn render_post(normalized_post: NormalizedPost, post: Post) -> Markup {
  let link_url = Url::parse(&normalized_post.link).ok();
  let json =
    serde_json::to_string_pretty(&post).unwrap_or_else(|e| e.to_string());
  let id = format!("entry-{}", rand_id());

  html! {
    article data-display-mode="rendered" data-folded="true" .post-entry {
      header {
        span .icon-container.fold-icon onclick="toggleFold(this)" title="Expand" {
          (sprite("caret-right"))
        }
        span .icon-container.raw-icon  onclick="toggleRaw(this)" title="HTML body" {
          (sprite("code"))
        }
        span .icon-container.json-icon  onclick="toggleJson(this)" title="JSON structure" {
          (sprite("json"))
        }

        div .flex style="margin-left: .5rem" {
          span .entry-title.grow { (normalized_post.title) }
          (external_link(&normalized_post.link))
        }
      }

      section {
        @if let Some(body) = &normalized_post.body {
          @let content = santize_html(body, link_url);
          div id=(id) .entry-content.rendered {
            template {
              style { (PreEscaped("*{max-width: 100%;}")) }
              (PreEscaped(content))
            }
            script { (PreEscaped(format!("fillEntryContent('{id}')"))) }
          }
          div .entry-content.raw {
            pre { code .language-html { (body) } }
          }
        } @else {
          div id=(id) .entry-content.rendered {
            "No body"
          }
          div .entry-content.raw {
            pre { code .language-html { } }
          }
        }

        div .entry-content.json {
          pre { code .language-json { (json) } }
        }
      }

      footer {
        @if let Some(date) = normalized_post.date {
          time .inline datetime=(date.to_rfc3339()) { (date.to_rfc2822()) }
        }
        @if let Some(author) = &normalized_post.author {
          span .author {
            (PreEscaped("By&nbsp;"));
            address .inline rel="author" { (author) }
          }
        }
      }
    }
  }
}

fn render_feed(mut feed: Feed, logs: Option<&[String]>) -> Markup {
  let normalized_feed = feed.normalize();
  let posts = feed.take_posts();

  html! {
    h3 .flex {
      (normalized_feed.title);
      (external_link(&normalized_feed.link))
    }
    @if let Some(description) = &normalized_feed.description {
      p { (description) }
    }

    @match logs {
      Some(logs) if !logs.is_empty() => {
        details .flash.error.logs {
          summary { "Logs" }
          div {
            @for log in logs {
              { pre { (log) } }
            }
          }
        }
      }
      _ => {}
    }

    p { (format!("Entries ({}):", normalized_feed.posts.len())) }

    @for (norm_post, post) in normalized_feed.posts.into_iter().zip(posts) {
      (render_post(norm_post, post))
    }
  }
}

fn inline_styles() -> Cow<'static, str> {
  super::Asset::get_content("endpoint.css")
}

fn inline_script() -> Cow<'static, str> {
  super::Asset::get_content("endpoint.js")
}

// requires the container to have a `display: flex` style
fn external_link(url: &str) -> Markup {
  html! {
    a style="margin-left: 0.25rem;display:inline-flex;align-self:center"
      href=(url)
      title="Open external link"
      target="_blank"
      rel="noopener noreferrer" {
      (sprite("external-link"))
    }
  }
}

fn filter_doc(filter_name: &str) -> Markup {
  html! {
    a style="margin-left: 0.25rem;display:inline-flex;align-self:center"
      href=(format!("https://github.com/shouya/rss-funnel/wiki/Filter-config#{}", filter_name))
      target="_blank"
      rel="noopener noreferrer"
      title="Documentation" {
      (sprite("book"))
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
