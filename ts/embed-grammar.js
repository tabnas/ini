#!/usr/bin/env node

// Embeds ini-grammar.jsonic into src/ini.ts and go/ini.go.
// Run via: npm run embed

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
embed(
  path.join(__dirname, '..', 'go', 'ini.go'),
  'const grammarText = `\n' + grammar + '`'
)
