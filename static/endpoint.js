function toggleFold(element) {
  const article = element.closest("article");
  article.dataset.folded = article.dataset.folded === "false";
}

function toggleRaw(element) {
  const article = element.closest("article");
  article.dataset.folded = "false";
  article.dataset.displayMode =
    article.dataset.displayMode === "raw" ? "rendered" : "raw";
}

function toggleJson(element) {
  const article = element.closest("article");
  article.dataset.folded = "false";
  article.dataset.displayMode =
    article.dataset.displayMode === "json" ? "rendered" : "json";
}

function gatherFilterSkip() {
  const skipped = [...document.querySelectorAll(".filter-item > .filter-name")]
    .filter((x) => !+x.dataset.enabled)
    .map((x) => x.dataset.index)
    .join(",");
  if (skipped === "") {
    return {};
  } else {
    return { filter_skip: skipped };
  }
}

function fillEntryContent(id) {
  const parent = document.getElementById(id);
  const shadowRoot = parent.attachShadow({ mode: "open" });
  const content = parent.querySelector("template").innerHTML;
  parent.innerHTML = "";
  shadowRoot.innerHTML = content;
}

function copyToClipboard() {
  const url = window.location.href.replace(/\/_\/endpoint\//, "/");
  navigator.clipboard.writeText(url);
  alert("Copied: " + url);
}
