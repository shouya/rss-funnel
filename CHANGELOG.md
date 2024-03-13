# Changelog

All notable changes to this project will be documented in this file.

## [0.1.1] - 2024-03-13

### Bug Fixes

- Mark `client` optional for merge filter (#77)
- Fix a typo and broken links in README

- Respect case-insensitive field of `keep_only`/`discard` filters (#79)

### Features

- `case_sensitive` option for `highlight` and `sanitize` filter (#82)

## [0.1.0-pre.4] - 2024-03-08

### Bug Fixes

- Specify default timeout for client

### Features

- Render HTML inside note filter

## [0.1.0-pre.1] - 2024-03-08

### Bug Fixes

- Fix typo in release workflow
- Handle non-utf8 feeds correctly (#67)

### Features

- Create feed from scratch (#44)
- Specify local endpoints as source (#47)
- Feature flag to disable inspector-ui on build time (#48)
- Add `note` filter (#50)
- Add `modify_post` and `modify_feed` filters (#51)
- Show filter docs (#55)
- Support merging multiple sources in parallel (#58)
- Optimize reload logic (#60)
- Show config and feed error message (#63)
- [**breaking**] Show json preview for the feed (#64)
- Add `convert_to` filter for converting feed format (#68)
- Implement fetch api for js runtime (#69)
- Authentication support (#70)
- Support early return in `modify_post` and `modify_feed` (#72)
- Render note filter's value as documentation (#73)
- Specify server flags via environment (#74)

### Performance

- Parallelize feed post-processing (#61)

## [0.0.5] - 2024-02-20

### Bug Fixes

- [**breaking**] Change content_type from endpoint config (#17)
- Fix interaction bugs (#27)
- Strip markup outside body from html sources (#31)

### Documentation

- Default bind adress in docker compose example (#21)
- Update README

### Features

- Improve text/xml and application/xml content type handling (#16)
- New `merge_feed` filter (#18)
- Add more DOM manipulation methods to the `Node` class (#20)
- Webui to inspect the feeds (#22)
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


