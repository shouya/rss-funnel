function setErrorMessage() {
  const message = document.getElementById("message");
  const search = window.location.search;
  if (search.includes("bad_auth=1")) {
    message.classList.remove("hidden");
    message.textContent = "Invalid username or password";
  } else if (search.includes("logged_out=1")) {
    message.classList.remove("hidden");
    message.textContent = "You have been logged out";
  } else if (search.includes("login_required=1")) {
    message.classList.remove("hidden");
    message.textContent = "You must be logged in to access that page";
  }
}
