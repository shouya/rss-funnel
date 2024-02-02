# Changelog

All notable changes to this project will be documented in this file.

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


