#!/usr/bin/env node

// Regenerates doc/grammar.svg and doc/grammar.txt from the LIVE grammar.
// Run via: npm run docs:grammar   (after npm run build)
//
// The @tabnas/railroad CLI cannot do this on its own: it loads a single
// plugin onto a bare Tabnas instance, and the ini plugin needs jsonic
// installed first (Hoover refuses to attach without a `val` rule). So the
// composition is done here and the model handed to the library API.

const fs = require('fs')
const path = require('path')

const { Tabnas } = require('@tabnas/parser')
const { jsonic } = require('@tabnas/jsonic')
const railroad = require('@tabnas/railroad')

const { Ini } = require('./dist/ini')

const outDir = path.join(__dirname, 'doc')

const tn = new Tabnas().use(jsonic).use(Ini)
const model = railroad.extractGrammar(tn)

fs.writeFileSync(path.join(outDir, 'grammar.svg'), railroad.modelToSvg(model))
fs.writeFileSync(path.join(outDir, 'grammar.txt'), railroad.modelToAscii(model))

console.log('wrote doc/grammar.svg and doc/grammar.txt for rules: ' +
  Object.keys(model.rules || {}).sort().join(', '))
