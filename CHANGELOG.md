# Changelog

All notable changes to this project will be documented in this file.

## [0.0.5-pre.1] - 2024-02-19

### Bug Fixes

- [**breaking**] Change content_type from endpoint config (#17)
- Fix interaction bugs (#27)
- Strip markup outside body from html sources (#31)

### Documentation

- Default bind adress in docker compose example (#21)

### Features

- Improve text/xml and application/xml content type handling (#16)
- New `merge_feed` filter (#18)
- Add more DOM manipulation methods to the `Node` class (#20)
- Webui to inspect the feeds (#22)
- Bump h2 from 0.3.22 to 0.3.24 (#23)

Bumps [h2](https://github.com/hyperium/h2) from 0.3.22 to 0.3.24.
<details>
<summary>Release notes</summary>
<p><em>Sourced from <a
href="https://github.com/hyperium/h2/releases">h2's
releases</a>.</em></p>
<blockquote>
<h2>v0.3.24</h2>
<h2>Fixed</h2>
<ul>
<li>Limit error resets for misbehaving connections.</li>
</ul>
<h2>v0.3.23</h2>
<h2>What's Changed</h2>
<ul>
<li>cherry-pick fix: streams awaiting capacity lockout in <a
href="https://redirect.github.com/hyperium/h2/pull/734">hyperium/h2#734</a></li>
</ul>
</blockquote>
</details>
<details>
<summary>Changelog</summary>
<p><em>Sourced from <a
href="https://github.com/hyperium/h2/blob/v0.3.24/CHANGELOG.md">h2's
changelog</a>.</em></p>
<blockquote>
<h1>0.3.24 (January 17, 2024)</h1>
<ul>
<li>Limit error resets for misbehaving connections.</li>
</ul>
<h1>0.3.23 (January 10, 2024)</h1>
<ul>
<li>Backport fix from 0.4.1 for stream capacity assignment.</li>
</ul>
</blockquote>
</details>
<details>
<summary>Commits</summary>
<ul>
<li><a
href="https://github.com/hyperium/h2/commit/7243ab5854b2375213a5a2cdfd543f1d669661e2"><code>7243ab5</code></a>
Prepare v0.3.24</li>
<li><a
href="https://github.com/hyperium/h2/commit/d919cd6fd8e0f4f5d1f6282fab0b38a1b4bf999c"><code>d919cd6</code></a>
streams: limit error resets for misbehaving connections</li>
<li><a
href="https://github.com/hyperium/h2/commit/a7eb14a487c0094187314fca63cfe4de4d3d78ef"><code>a7eb14a</code></a>
v0.3.23</li>
<li><a
href="https://github.com/hyperium/h2/commit/b668c7fbe22e0cb4a76b0a67498cbb4d0aacbc75"><code>b668c7f</code></a>
fix: streams awaiting capacity lockout (<a
href="https://redirect.github.com/hyperium/h2/issues/730">#730</a>) (<a
href="https://redirect.github.com/hyperium/h2/issues/734">#734</a>)</li>
<li>See full diff in <a
href="https://github.com/hyperium/h2/compare/v0.3.22...v0.3.24">compare
view</a></li>
</ul>
</details>
<br />


[![Dependabot compatibility
score](https://dependabot-badges.githubapp.com/badges/compatibility_score?dependency-name=h2&package-manager=cargo&previous-version=0.3.22&new-version=0.3.24)](https://docs.github.com/en/github/managing-security-vulnerabilities/about-dependabot-security-updates#about-compatibility-scores)

Dependabot will resolve any conflicts with this PR as long as you don't
alter it yourself. You can also trigger a rebase manually by commenting
`@dependabot rebase`.

[//]: # (dependabot-automerge-start)
[//]: # (dependabot-automerge-end)

---

<details>
<summary>Dependabot commands and options</summary>
<br />

You can trigger Dependabot actions by commenting on this PR:
- `@dependabot rebase` will rebase this PR
- `@dependabot recreate` will recreate this PR, overwriting any edits
that have been made to it
- `@dependabot merge` will merge this PR after your CI passes on it
- `@dependabot squash and merge` will squash and merge this PR after
your CI passes on it
- `@dependabot cancel merge` will cancel a previously requested merge
and block automerging
- `@dependabot reopen` will reopen this PR if it is closed
- `@dependabot close` will close this PR and stop Dependabot recreating
it. You can achieve the same result by closing it manually
- `@dependabot show <dependency name> ignore conditions` will show all
of the ignore conditions of the specified dependency
- `@dependabot ignore this major version` will close this PR and stop
Dependabot creating any more for this major version (unless you reopen
the PR or upgrade to it yourself)
- `@dependabot ignore this minor version` will close this PR and stop
Dependabot creating any more for this minor version (unless you reopen
the PR or upgrade to it yourself)
- `@dependabot ignore this dependency` will close this PR and stop
Dependabot creating any more for this dependency (unless you reopen the
PR or upgrade to it yourself)
You can disable automated security fix PRs for this repo from the
[Security Alerts
page](https://github.com/shouya/rss-funnel/network/alerts).

</details>

Signed-off-by: dependabot[bot] <support@github.com>
Co-authored-by: dependabot[bot] <49699333+dependabot[bot]@users.noreply.github.com>
- Add an option to disable inspector ui (#28)
- Auto reload server on config change (#29)
- Add a reload button (#30)

### Testing

- Add config and feature tests (#13)
- Read from fixture directory (#19)

## [0.0.4] - 2024-02-02

### Bug Fixes

- Fix multiarch build: push manifest

- Fix node mutation not working on DOM
- Avoid including <html> tag in set_{inner,outer}_html

### Features

- Set_attr and unset_attr methods for Node
- Add more dom manipulation methods
- Add select method to DOM Node
- Add Node.children() method
- Add post selection filters (keep_only/discard)
- Add highlighter filter (#10)
- Caching requests to servers for feed (#12)

### Testing

- Add various tests for the DOM API

## [0.0.3] - 2024-01-22

### Features

- Support for limiting the filter steps

- Prettify xml support

- Endpoint testing support

- Multiarch support


## [0.0.2] - 2024-01-19

### Bug Fixes

- Fix atom feed escaping in serialization


### Features

- Support specifying version and image host in Makefile

- Atom feed support


## [0.0.1] - 2024-01-09

### Bug Fixes

- Fix relative link in split filter

- Fix post deserialization

- Fix endpoints with dynamic source

- Fix erros and improve languages in README

- Fix content vs description field

- Fix error in README


### Features

- Support https

- Support console.log in js runtime

- Import from http support for js runtime

- Feed splitting support

- Add dynamic source support

- Support modifying posts in js filter

- Support text/xml mime type


