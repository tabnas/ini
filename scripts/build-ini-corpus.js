#!/usr/bin/env node
/* Build test/corpus/ini-corpus.json from the fetched third-party upstreams.
 *
 * Run by scripts/fetch-ini-corpus.sh. Both runtimes read the generated
 * manifest, so the valid/invalid labelling and the expected values are
 * decided ONCE, here, and cannot drift between TypeScript and Go.
 *
 * ---------------------------------------------------------------------
 * WHY THERE IS NO AUTHORITATIVE SUITE, AND WHAT THIS IS INSTEAD
 * ---------------------------------------------------------------------
 * INI has no specification. There is no cross-implementation conformance
 * suite for it (no toml-test / JSONTestSuite / yaml-test-suite analogue).
 * What exists is the private test data of individual implementations.
 *
 * So: the DOCUMENTS are third-party (four upstreams, pinned SHAs). The
 * EXPECTED VALUES for the valid half come from running the pinned
 * npm/ini implementation over the same documents -- a differential
 * oracle, not a standard.
 *
 * npm/ini is the right oracle for this plugin specifically because
 * @tabnas/ini targets npm/ini's dialect and output model, feature for
 * feature: parse-to-plain-object, string values, `key[] =` repeated-key
 * arrays, `[a.b.c]` dotted section nesting, `[x\.y]` escaped-dot section
 * names, bare keys resolving to `true`, `true`/`false`/`null` keywords.
 * No other INI implementation has that combination. @tabnas/ini's own
 * shared fixtures in test/spec/ encode exactly these npm/ini semantics.
 *
 * This is stated plainly rather than dressed up as a standard. A
 * disagreement with the oracle is evidence, not proof of a bug -- but
 * every disagreement is a place where two INI parsers turn the same
 * bytes into different objects, which is what the dial is measuring.
 *
 * ---------------------------------------------------------------------
 * THE INVALID HALF, AND ITS LIMIT
 * ---------------------------------------------------------------------
 * The invalid half is small, and that is a property of INI, not a
 * narrowing of the corpus. Only documents that the upstream shipping
 * them explicitly labels as SYNTAX ERRORS are counted must-fail.
 *
 * Read this before trusting the invalid number: the npm/ini oracle
 * ACCEPTS ALL SIX of them (npm/ini essentially never errors -- an
 * unparseable line becomes a bare key set to `true`). Each case records
 * `oracleAccepts`, so the disagreement is visible rather than hidden.
 * This half therefore measures strictness relative to the C
 * implementations (inih, iniparser), which is a real question for a
 * parser advertising itself as an INI parser, but it is not a verdict
 * from an undisputed standard.
 *
 * Two files upstream files in iniparser's `bad_ini/` directory are
 * deliberately NOT counted must-fail -- twisted-ofkey.ini and
 * twisted-ofval.ini are labelled bad because they overflow iniparser's
 * fixed line buffer, not because they are syntactically wrong. Counting
 * a 10KB-but-well-formed key as must-fail would manufacture a failure.
 * They are in the valid half.
 */
'use strict'

const fs = require('fs')
const path = require('path')
const crypto = require('crypto')

const REPO = path.join(__dirname, '..')
const CORPUS = path.join(REPO, 'test', 'corpus')
const UPSTREAM = path.join(CORPUS, 'upstream')
const OUT = path.join(CORPUS, 'ini-corpus.json')

// The single source of truth for what is fetched and at which commit.
// scripts/fetch-ini-corpus.js imports this, so there is only one pin.
// Pinned to exact commit SHAs, never a branch. Bump deliberately, and
// re-read the dial when you do.
const UPSTREAMS = {
  'inih': {
    url: 'https://github.com/benhoyt/inih',
    sha: '577ae2dee1f0d9c2d11c7f10375c1715f3d6940c',
  },
  'npm-ini': {
    url: 'https://github.com/npm/ini',
    sha: '3c96c74fd42584bd655e17a4e63e2ef0a3b406ee',
  },
  'iniparser': {
    url: 'https://github.com/ndevilla/iniparser',
    sha: '4bef811283e0ec1658c60e09950bd5a1ddc92e4b',
  },
  'go-ini': {
    url: 'https://github.com/go-ini/ini',
    sha: '2045e714610fdab6c659c937154732ca28f65213',
  },
}

// Documents the shipping upstream labels a SYNTAX error, with the evidence.
// Anything not listed here is in the valid half.
const MUST_FAIL = {
  'inih/tests/bad_section.ini':
    'inih tests/baseline_single.txt records e=3 (error on line 3): "[section2" is an unterminated section header',
  'inih/tests/bad_comment.ini':
    'inih tests/baseline_single.txt records e=1: line 1 "This is an error" is neither section, comment nor name=value',
  'inih/tests/bad_multi.ini':
    'inih ships it as bad_*: a leading indented continuation line with no preceding name=value',
  'inih/tests/name_only_after_error.ini':
    'the file\'s own comment: "Deliberately cause an error due to unterminated section" ([broken)',
  'iniparser/test/ressources/bad_ini/ends_well.ini':
    'shipped in iniparser\'s bad_ini/ directory; line 5 "error is here" has no separator',
  'iniparser/test/ressources/bad_ini/twisted-errors.ini':
    'shipped in iniparser\'s bad_ini/ directory; its own comment: "All of these should trigger syntax errors"',
}

// In bad_ini/ upstream, but NOT syntax errors -- see the header comment.
const NOT_MUST_FAIL_REASON = {
  'iniparser/test/ressources/bad_ini/twisted-ofkey.ini':
    'in bad_ini/ for buffer-overflow reasons (a ~10KB key), not syntax; a parser accepting it is not wrong',
  'iniparser/test/ressources/bad_ini/twisted-ofval.ini':
    'in bad_ini/ for buffer-overflow reasons (a ~10KB value), not syntax; a parser accepting it is not wrong',
}

function walk(dir, out = []) {
  if (!fs.existsSync(dir)) return out
  for (const e of fs.readdirSync(dir).sort()) {
    if (e === '.git') continue
    const p = path.join(dir, e)
    const st = fs.lstatSync(p)
    if (st.isDirectory()) walk(p, out)
    else if (st.isFile() && p.endsWith('.ini')) out.push(p)
  }
  return out
}

function build() {
  const missing = Object.keys(UPSTREAMS).filter(
    (n) => !fs.existsSync(path.join(UPSTREAM, n))
  )
  if (missing.length) {
    console.error(
      'ini corpus: upstream checkout(s) missing: ' + missing.join(', ') +
      '\nRun ./scripts/fetch-ini-corpus.sh first.'
    )
    process.exit(1)
  }

  // The oracle, from the pinned npm/ini checkout. Zero dependencies.
  const oraclePath = path.join(UPSTREAM, 'npm-ini', 'lib', 'ini.js')
  if (!fs.existsSync(oraclePath)) {
    console.error('ini corpus: npm/ini oracle not found at ' + oraclePath)
    process.exit(1)
  }
  const oracle = require(oraclePath)

  // Collect every document first. Several upstreams ship the same bytes at
  // two paths (iniparser has example/x.ini == test/ressources/bad_ini/x.ini);
  // dedupe by content, but keep the copy that carries an explicit label,
  // otherwise the labelled path loses the coin toss and a must-fail case
  // silently becomes a valid one.
  const all = []
  for (const name of Object.keys(UPSTREAMS).sort()) {
    for (const file of walk(path.join(UPSTREAM, name))) {
      const rel = name + '/' + path.relative(path.join(UPSTREAM, name), file)
        .split(path.sep).join('/')
      all.push({ name, file, rel })
    }
  }
  all.sort((a, b) => {
    const la = MUST_FAIL[a.rel] || NOT_MUST_FAIL_REASON[a.rel] ? 0 : 1
    const lb = MUST_FAIL[b.rel] || NOT_MUST_FAIL_REASON[b.rel] ? 0 : 1
    return la - lb || a.rel.localeCompare(b.rel)
  })

  const seen = new Map() // content hash -> first case name
  const cases = []

  {
    for (const { name, file, rel } of all) {
      const buf = fs.readFileSync(file)
      const hash = crypto.createHash('sha256').update(buf).digest('hex')
      if (seen.has(hash)) continue // byte-identical duplicate across upstreams
      seen.set(hash, rel)

      const source = buf.toString('utf8')
      const kind = MUST_FAIL[rel] ? 'invalid' : 'valid'

      let oracleAccepts = true
      let expected = null
      try {
        expected = JSON.parse(JSON.stringify(oracle.parse(source)))
      } catch (e) {
        oracleAccepts = false
        expected = null
      }

      if (kind === 'valid' && !oracleAccepts) {
        // The oracle cannot produce an expected value, so there is nothing
        // to assert against. Refuse rather than emit an unassertable case.
        console.error(
          'ini corpus: oracle failed on valid case ' + rel + ' -- ' +
          'classify it explicitly in MUST_FAIL or fix the builder.'
        )
        process.exit(1)
      }

      cases.push({
        name: rel,
        upstream: name,
        upstreamUrl: UPSTREAMS[name].url,
        upstreamSha: UPSTREAMS[name].sha,
        kind,
        source,
        sourceSha256: hash,
        expected: kind === 'valid' ? expected : null,
        oracleAccepts,
        label: kind === 'invalid'
          ? MUST_FAIL[rel]
          : (NOT_MUST_FAIL_REASON[rel] || 'not labelled a syntax error upstream'),
      })
    }
  }

  cases.sort((a, b) => a.name.localeCompare(b.name))

  // A typo'd MUST_FAIL key would silently shrink the must-fail half, which is
  // exactly the kind of quiet weakening this harness exists to prevent.
  const emitted = new Set(cases.map((c) => c.name))
  const orphan = [...Object.keys(MUST_FAIL), ...Object.keys(NOT_MUST_FAIL_REASON)]
    .filter((k) => !emitted.has(k))
  if (orphan.length) {
    console.error(
      'ini corpus: labelled path(s) matched no emitted document: ' +
      orphan.join(', ') +
      '\nThe upstream layout changed, or the path is misspelt. Refusing to ' +
      'emit a corpus with a silently-dropped label.'
    )
    process.exit(1)
  }

  const validN = cases.filter((c) => c.kind === 'valid').length
  const invalidN = cases.length - validN

  const manifest = {
    suite: 'tabnas-ini third-party corpus (assembled; INI has no authoritative suite)',
    oracle: {
      name: 'npm/ini',
      url: UPSTREAMS['npm-ini'].url,
      sha: UPSTREAMS['npm-ini'].sha,
      note: 'expected values for the valid half are this implementation\'s output',
    },
    upstreams: UPSTREAMS,
    generatedBy: 'scripts/build-ini-corpus.js',
    counts: { valid: validN, invalid: invalidN, total: cases.length },
    cases,
  }

  fs.mkdirSync(CORPUS, { recursive: true })
  fs.writeFileSync(OUT, JSON.stringify(manifest, null, 2) + '\n')
  console.log(
    'wrote ' + path.relative(REPO, OUT) + ': ' +
    validN + ' valid / ' + invalidN + ' invalid (' + cases.length + ' documents)'
  )
}

module.exports = { UPSTREAMS, MUST_FAIL, build }

if (require.main === module) build()
