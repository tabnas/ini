# @jsonic/ini

A [Jsonic](https://github.com/jsonicjs/jsonic) syntax plugin that
parses [INI](https://github.com/microsoft/node-ini-parser) format
files into objects with support for sections, nested keys, arrays,
multiline values, and inline comments.

Available for [TypeScript](doc/ini-ts.md) and [Go](../go/doc/ini-go.md).

[![npm version](https://img.shields.io/npm/v/@jsonic/ini.svg)](https://npmjs.com/package/@jsonic/ini)
[![build](https://github.com/jsonicjs/ini/actions/workflows/build.yml/badge.svg)](https://github.com/jsonicjs/ini/actions/workflows/build.yml)
[![Coverage Status](https://coveralls.io/repos/github/jsonicjs/ini/badge.svg?branch=main)](https://coveralls.io/github/jsonicjs/ini?branch=main)
[![Known Vulnerabilities](https://snyk.io/test/github/jsonicjs/ini/badge.svg)](https://snyk.io/test/github/jsonicjs/ini)
[![DeepScan grade](https://deepscan.io/api/teams/5016/projects/25267/branches/788638/badge/grade.svg)](https://deepscan.io/dashboard#view=project&tid=5016&pid=25267&bid=788638)
[![Maintainability](https://api.codeclimate.com/v1/badges/6da148ebd83e336cdcbe/maintainability)](https://codeclimate.com/github/jsonicjs/ini/maintainability)

| ![Voxgig](https://www.voxgig.com/res/img/vgt01r.png) | This open source module is sponsored and supported by [Voxgig](https://www.voxgig.com). |
| ---------------------------------------------------- | --------------------------------------------------------------------------------------- |


## Tutorials

Learn by building working examples from scratch.

- [Parse a basic INI file (TypeScript)](doc/ini-ts.md#parse-a-basic-ini-file)
- [Parse a basic INI file (Go)](../go/doc/ini-go.md#parse-a-basic-ini-file)
- [Parse nested sections and arrays (TypeScript)](doc/ini-ts.md#parse-nested-sections-and-arrays)
- [Parse nested sections and arrays (Go)](../go/doc/ini-go.md#parse-nested-sections-and-arrays)
- [Parse with multiline values (TypeScript)](doc/ini-ts.md#parse-with-multiline-values)
- [Parse with multiline values (Go)](../go/doc/ini-go.md#parse-with-multiline-values)


## How-to guides

Solve specific problems with INI configuration.

- [Enable inline comments (TypeScript)](doc/ini-ts.md#enable-inline-comments) | [(Go)](../go/doc/ini-go.md#enable-inline-comments)
- [Escape inline comment characters (TypeScript)](doc/ini-ts.md#escape-inline-comment-characters) | [(Go)](../go/doc/ini-go.md#escape-inline-comment-characters)
- [Control duplicate section handling (TypeScript)](doc/ini-ts.md#control-duplicate-section-handling) | [(Go)](../go/doc/ini-go.md#control-duplicate-section-handling)
- [Use boolean keys (TypeScript)](doc/ini-ts.md#use-boolean-keys) | [(Go)](../go/doc/ini-go.md#use-boolean-keys)


## Explanation

Understand how the INI parser works under the hood.

- [How INI parsing works (TypeScript)](doc/ini-ts.md#how-ini-parsing-works) | [(Go)](../go/doc/ini-go.md#how-ini-parsing-works)
- [Value resolution order (TypeScript)](doc/ini-ts.md#value-resolution-order) | [(Go)](../go/doc/ini-go.md#value-resolution-order)


## Reference

Complete API documentation for each language.

- [TypeScript API reference](doc/ini-ts.md#reference)
- [Go API reference](../go/doc/ini-go.md#reference)


## Quick example

Parse an INI file with sections, keys, and arrays:

**TypeScript**
```typescript
const j = Jsonic.make().use(Ini)

j("[database]\nhost = localhost\nport = 5432\ntags[] = primary\ntags[] = read")
// { database: { host: 'localhost', port: '5432', tags: ['primary', 'read'] } }
```

**Go**
```go
result, err := ini.Parse("[database]\nhost = localhost\nport = 5432\ntags[] = primary\ntags[] = read")
// map[string]any{"database": map[string]any{"host": "localhost", "port": "5432", "tags": []any{"primary", "read"}}}
```


## License

MIT. Copyright (c) Richard Rodger and other contributors.
