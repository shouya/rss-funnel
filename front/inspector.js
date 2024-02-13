
class FeedInspector {
  constructor() {
    this.current_endpoint = null;
    this.preview = null;
  }
}

function setup_endpoint_callback() {
  const nodes = document.querySelectorAll(".endpoint-list .endpoint");
  for (const node of nodes) {
    node.addEventListener("click", load_preview);
  }
}

function load_preview() {
  this.classList.add("loading");
  // remove the "/" prefix
  const path = this.querySelector(".endpoint-path").innerText.slice(1);
  fetch(`/_inspector/preview/${path}`)
    .then((response) => response.text())
    .then((text) => {
      const element = document.getElementById("feed-preview");
      if (!window.editor) {
        window.editor = CodeMirror(element, {
          readOnly: true,
          editable: false,
          mode: "text/xml",
          wrapMethod: "code",
          lineNumbers: true,
          foldGutter: true,
          gutters: ["CodeMirror-foldgutter"],
        });
      }
      window.editor.setValue(text);

      this.classList.remove("loading");
    })
  ;
}

document.addEventListener("DOMContentLoaded", setup_endpoint_callback);
