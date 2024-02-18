import { elt, $ } from "./util.js";
import { Filter } from "./Filter.js";
import { basicSetup, EditorView } from "codemirror";
import { EditorState } from "@codemirror/state";
import { xml } from "@codemirror/lang-xml";
import Split from "split.js";
import HtmlSanitizer from "jitbit-html-sanitizer";

export class FeedInspector {
  constructor() {
    this.config = null;
    this.current_endpoint = null;
    this.current_preview = null;
    this.raw_editor = null;
    this.raw_feed_xml = null;
  }

  async init() {
    await this.setup_raw_editor();
    await this.setup_splitter();
    await this.setup_view_mode_selector();

    const resp = await fetch("/_inspector/config");
    this.config = await resp.json();

    await Promise.all([this.load_endpoints(), this.setup_param()]);
  }

  async setup_view_mode_selector() {
    $("#view-mode-selector #rendered-radio").addEventListener("change", () => {
      this.render_feed();
    });

    $("#view-mode-selector #raw-radio").addEventListener("change", () => {
      this.render_feed();
    });
  }

  async setup_param() {
    [
      $("#request-param #source"),
      $("#request-param #limit-posts"),
      $("#request-param #limit-posts-checkbox"),
    ].forEach((input) => {
      input.addEventListener("change", () => this.fetch_and_render_feed());
    });

    $("#request-param #limit-filters").addEventListener("change", () => {
      this.render_filters();
      this.fetch_and_render_feed();
    });
    $("#request-param #limit-filters-checkbox").addEventListener(
      "change",
      () => {
        this.render_filters();
        this.fetch_and_render_feed();
      },
    );
  }

  async setup_raw_editor() {
    $("#feed-preview").classList.add("hidden");
    this.raw_editor = new EditorView({
      extensions: [
        basicSetup,
        xml(),
        EditorState.readOnly.of(true),
        EditorView.lineWrapping,
      ],
      parent: $("#feed-preview #raw"),
    });
  }

  async setup_splitter() {
    Split(["#sidebar-panel", "#main-panel"], {
      sizes: [20, 80],
      snapOffset: 0,
      gutterSize: 3,
      minSize: [300, 500],
    });
  }

  async load_endpoints() {
    $("#sidebar-filters").classList.add("hidden");
    $("#sidebar-endpoints").classList.remove("hidden");
    $("#endpoint-list").innerHTML = "";

    for (const endpoint of this.config.endpoints) {
      const path_node = elt("div", { class: "endpoint-path" }, endpoint.path);
      path_node.addEventListener("click", () => {
        this.current_endpoint = endpoint;
        this.load_endpoint();
      });
      const copy_url_node = elt(
        "div",
        { class: "button", href: endpoint.path },
        "Copy URL",
      );
      copy_url_node.addEventListener("click", (e) => {
        e.preventDefault();
        this.copy_endpoint_url(endpoint);
      });

      const node = elt("li", { class: "endpoint" }, [
        elt("div", { class: "endpoint-header" }, [path_node, copy_url_node]),
        endpoint.note && elt("div", { class: "endpoint-note" }, endpoint.note),
      ]);
      $("#endpoint-list").appendChild(node);
    }
    $("#sidebar-endpoints").classList.remove("hidden");
  }

  render_filters() {
    if (!this.current_endpoint) return;
    const { filters } = this.current_endpoint;
    const limit =
      $("#limit-filters-checkbox").checked && $("#limit-filters").value;
    const filter_ul_node = $("#filter-list");
    filter_ul_node.innerHTML = "";
    for (const [index, filter] of filters.entries()) {
      const [name, conf] = Object.entries(filter)[0];
      const node = new Filter(name, conf).render_node();

      if (limit !== false && index >= limit) {
        node.classList.add("inactive");
      }

      filter_ul_node.appendChild(node);
    }
  }

  async fetch_and_render_feed() {
    await this.fetch_feed();
    this.render_feed();
  }

  async render_feed() {
    const view_mode =
      ($("#view-mode-selector #rendered-radio-input").checked && "rendered") ||
      ($("#view-mode-selector #raw-radio-input").checked && "raw") ||
      "rendered";

    ["rendered", "raw"].forEach((mode) => {
      if (mode === view_mode) {
        $(`#feed-preview #${mode}`).classList.remove("hidden");
      } else {
        $(`#feed-preview #${mode}`).classList.add("hidden");
      }

      const raw_feed_xml_xml = this.raw_feed_xml;
      const function_name = `render_feed_${mode}`;
      if (this[function_name]) {
        this[function_name](raw_feed_xml_xml);
      }
    });
  }

  async render_feed_rendered(raw_feed_xml_xml) {
    const parsed = parse_feed(raw_feed_xml_xml);
    if (!parsed) {
      console.error("Failed to parse feed");
      return;
    }

    $("#feed-preview #rendered").innerHTML = "";
    const title_node = elt("h2", { class: "feed-title" }, parsed.title);
    $("#feed-preview #rendered").appendChild(title_node);

    const sanitizer = new HtmlSanitizer({});

    for (const post of parsed.posts) {
      const post_content = elt("p", { class: "feed-post-content" }, []);
      post_content.innerHTML = sanitizer.sanitizeHtml(post.content || "");
      const post_node = elt("div", { class: "feed-post" }, [
        elt(
          "h3",
          { class: "feed-post-title" },
          elt("a", { class: "feed-post-link", href: post.link }, post.title),
        ),
        post_content,
        elt("p", { class: "feed-post-date" }, post.date),
      ]);
      $("#feed-preview #rendered").appendChild(post_node);
    }
  }

  async render_feed_raw(raw_feed_xml_xml) {
    if (this.raw_editor.state.doc.toString() === raw_feed_xml_xml) {
      return;
    }
    this.raw_editor.dispatch({
      changes: {
        from: 0,
        to: this.raw_editor.state.doc.length,
        insert: raw_feed_xml_xml,
      },
    });
  }

  async load_endpoint() {
    if (!this.current_endpoint) return;
    const { path, source, filters } = this.current_endpoint;

    // switch sidebar ui
    $("#sidebar-endpoints").classList.add("hidden");
    $("#endpoint-name").textContent = path;
    $("#back-to-endpoints").addEventListener("click", () => {
      this.load_endpoints();
    });
    $("#copy-endpoint-url").addEventListener("click", () => {
      this.copy_endpoint_url(this.current_endpoint);
    });
    $("#sidebar-filters").classList.remove("hidden");

    // switch main ui
    $("input#source", $("#request-param")).disabled = !!source;
    if (source) {
      $("input#source", $("#request-param")).placeholder = source;
      $("input#source", $("#request-param")).value = "";
    } else {
      $("input#source", $("#request-param")).placeholder =
        "Source not configured. Please specify it here.";
    }
    $("#request-param").classList.remove("hidden");

    // update parameter input
    $("#limit-filters").setAttribute("max", filters.length);
    $("#limit-filters").value = filters.length;
    $("#limit-filters-checkbox").checked = false;

    // show filter list
    this.render_filters();

    // show feed preview
    await this.fetch_feed();
    this.render_feed();
  }

  async fetch_feed() {
    if (!this.current_endpoint) return;
    const { path } = this.current_endpoint;

    const params = this.feed_request_param();
    $("#feed-preview").classList.remove("hidden");
    $("#feed-preview").classList.add("loading");

    const time_start = performance.now();
    const request_path = `${path}?${params}`;
    $("#fetch-status").innerText = `Fetching ${request_path}...`;
    const resp = await fetch(`${path}?${params}`);
    const text = await resp.text();

    $("#fetch-status").innerText = `Fetched ${request_path} in ${
      performance.now() - time_start
    }ms.`;

    $("#feed-preview").classList.remove("loading");

    this.raw_feed_xml = text;
  }

  feed_request_param() {
    const parent = $("#request-param");
    const source = $("#source", parent).value;
    const limit_posts =
      $("#limit-posts-checkbox", parent).checked &&
      $("#limit-posts", parent).value;
    const limit_filters =
      $("#limit-filters-checkbox", parent).checked &&
      $("#limit-filters", parent).value;

    const params = [];
    if (source) params.push(`source=${source}`);
    if (limit_posts) params.push(`limit_posts=${limit_posts}`);
    if (limit_filters) params.push(`limit_filters=${limit_filters}`);

    params.push("pp=1");
    return params.join("&");
  }

  full_feed_url(endpoint) {
    const parent = $("#request-param");
    const source = $("#source", parent).value;
    let url = new URL(endpoint.path, window.location);

    if (!endpoint.source) {
      url.searchParams.set("source", source);
    }

    return url.href;
  }

  copy_endpoint_url(endpoint) {
    const url = this.full_feed_url(endpoint);
    navigator.clipboard.writeText(url);
    const node = elt("div", { class: "popup-alert", style: "opacity: 1" }, [
      elt("div", { class: "alert-header" }, "URL Copied"),
      elt("div", { class: "alert-body" }, url),
    ]);
    document.body.appendChild(node);
    setTimeout(() => (node.style.opacity = 0), 3000);
    setTimeout(() => node.remove(), 4000);
  }
}

// return {title: string, posts: [post]}
// post: {title: string, link: string, date: string, content: string}
function parse_feed(xml) {
  const parser = new DOMParser();
  const doc = parser.parseFromString(xml, "text/xml");

  if (doc.documentElement.tagName == "rss") {
    const title = doc.querySelector("channel > title").textContent.trim();
    const posts = Array.from(doc.querySelectorAll("item")).map((item) => {
      return {
        title: item.querySelector("title")?.textContent?.trim(),
        link: item.querySelector("link")?.textContent?.trim(),
        date: item.querySelector("pubDate")?.textContent?.trim(),
        content: item.querySelector("description")?.textContent?.trim(),
      };
    });

    return { title, posts };
  } else if (doc.documentElement.tagName == "feed") {
    const title = doc.querySelector("feed > title").textContent.trim();
    const posts = Array.from(doc.querySelectorAll("entry")).map((entry) => {
      return {
        title: entry.querySelector("title")?.textContent?.trim(),
        link: entry.querySelector("link")?.getAttribute("href"),
        date: entry.querySelector("published")?.textContent?.trim(),
        content: entry.querySelector("content")?.textContent?.trim(),
      };
    });

    return { title, posts };
  }

  return null;
}
