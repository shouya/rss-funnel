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
    .then((html) => {
      document.querySelector(".feed-preview").innerHTML = html;
      this.classList.remove("loading");
    });
}

document.addEventListener("DOMContentLoaded", setup_endpoint_callback);
