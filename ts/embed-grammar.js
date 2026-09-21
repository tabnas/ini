#!/usr/bin/env node

// Embeds ini-grammar.jsonic into src/ini.ts, go/ini.go and
// rs/src/lib.rs.
// Run via: npm run embed
//
// All three runtimes embed the grammar as the jsonic TEXT it is
// authored in, and each parses it with its own jsonic at load time, so
// they do not each have a grammar: they have THE grammar.
//
// Never hand-edit between the BEGIN/END markers: edit
// ini-grammar.jsonic and re-run this script.

const fs = require('fs')
const path = require('path')

const grammar = fs.readFileSync(path.join(__dirname, '..', 'ini-grammar.jsonic'), 'utf8')

const BEGIN = '// --- BEGIN EMBEDDED ini-grammar.jsonic ---'
const END = '// --- END EMBEDDED ini-grammar.jsonic ---'

function embed(file, wrapContent) {
  let src = fs.readFileSync(file, 'utf8')
  const beginIdx = src.indexOf(BEGIN)
  const endIdx = src.indexOf(END)
  if (beginIdx === -1 || endIdx === -1) {
    console.error('Error: embedding markers not found in ' + file)
    process.exit(1)
  }
  const replacement = BEGIN + '\n' + wrapContent + '\n' + END
  src = src.substring(0, beginIdx) + replacement + src.substring(endIdx + END.length)
  fs.writeFileSync(file, src)
}

// TypeScript: template literal (escape backslashes, backticks, ${).
const tsContent = grammar
  .replace(/\\/g, '\\\\')
  .replace(/`/g, '\\`')
  .replace(/\$\{/g, '\\${')
embed(
  path.join(__dirname, 'src', 'ini.ts'),
  'const grammarText = `\n' + tsContent + '`'
)

// Go: raw string (backticks cannot appear in content).
if (grammar.includes('`')) {
  console.error('Error: grammar file contains backticks, cannot embed in Go raw string')
  process.exit(1)
}
// The trailing newline leaves a blank line before END, which keeps the
// result gofmt-clean (gofmt wants a blank line between the const
// declaration and the trailing comment).
embed(
  path.join(__dirname, '..', 'go', 'ini.go'),
  'const grammarText = `\n' + grammar + '`\n'
)

// Rust: raw string. A raw string has no escapes either, so the grammar
// goes in verbatim, but the hash count has to clear the longest `"#...`
// run the content holds. There is none, so one hash is enough; say so
// rather than emit a literal that will not compile.
const RS_FILE = path.join(__dirname, '..', 'rs', 'src', 'lib.rs')
if (fs.existsSync(RS_FILE)) {
  if (grammar.includes('"#')) {
    console.error('Error: grammar file contains `"#`, cannot embed in an r#"..."# raw string')
    process.exit(1)
  }
  embed(RS_FILE, 'const GRAMMAR_TEXT: &str = r#"\n' + grammar + '"#;')
} else {
  console.log('No Rust source at', RS_FILE, '- skipping')
}
