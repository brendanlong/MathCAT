# Speech performance experiments

Measurements of where `set_mathml` + `get_spoken_text` spend their time, and of some changes that
make them faster without changing the rule format or any output. Nothing here is proposed upstream yet.

## How it was measured

`examples/bench_speech.rs` reads a file with one `<math>` element per line, converts each to speech
(English, ClearSpeak, rules read from the `Rules` directory), and reports the fastest of several passes
after a warm-up call. Two inputs, neither checked in:

- **wiki**: all 605 `<math>` elements (16,776 MathML elements) from the Wikipedia articles on Maxwell's
  equations, the Schrödinger equation and the Navier–Stokes equations, annotations removed.
- **lw**: the 107 `<math>` elements (1,168 MathML elements) of
  <https://www.lesswrong.com/posts/THgxsiGm7eeqkGsTB/need-for-speed>, the post Lion Reader's narration was tested on.

Every variant produced byte-identical speech for all 712 equations, and the full test suite passes with
all of the changes applied (8,365 tests).

## Results

Microseconds per equation, each row including the rows above it:

| Change | Where | wiki | lw |
|---|---|---:|---:|
| `main` (1b27bde0) | | 940 | 363 |
| 1. Sort nodes into document order without indexing the whole document | sxd-xpath | 582 | 312 |
| 2. FxHash instead of SipHash for node sets and function/variable tables | sxd-xpath | 512 | 275 |
| 3. No allocation to look up a function or variable; no hash set to compare a node set to a string | sxd-xpath | 489 | 261 |
| 4. Small node sets stored as a vector instead of a hash set | sxd-xpath | 461 | 254 |
| 5. Prefilter: skip a rule when a cheap check shows its `match` can't succeed | MathCAT | 398 | 220 |
| 6. `CheckRuleFiles` preference set to `None` (no code change) | caller | 381 | 207 |

That is 2.5× faster on the long equations and 1.75× on the short ones. The whole LessWrong post goes from
39 ms to 22 ms. Loading the rules on the first call takes another 12 ms (27 ms with `include-zip`).

Changes 1–4 are in `sxd-xpath-patches/`, as commits against <https://github.com/nsoiffer/sxd-xpath>
(`git am` them onto `master`, then build MathCAT with
`--config 'patch.crates-io.sxd-xpath-no-unsafe.path="../sxd-xpath"'`). Change 5 is in `src/speech.rs` on this branch.

## What the time is spent on

Rules are already parsed and their XPath compiled once, when the rule files are first read, and each
equation is parsed once. The time goes to *evaluating* the compiled XPath:

- For each element, the rules for its tag are tried in order until one matches. On the wiki input that is
  315,000 `match` evaluations for 29,500 matches, about 11 tries per match, at 0.4–2 µs per try. An `mrow`
  goes through about 27 intent rules before reaching the default; an `mi` goes through 18 speech rules
  (`.='sin'`, `.='cos'`, ...).
- Before change 1, 38% of all time was one function: every time a rule replaces an element with several of
  its children, sxd-xpath put them in document order by building a hash map of *every node in the document*.
- The XPath interpreter allocates heavily. Evaluating `.='sinh'` built a hash set holding the one context node,
  cloned the literal, and built a second hash set of strings to compare against.
- The single most expensive rule is `turn-off` in `intent.yaml`. It is tried on every element and walks
  all of the element's ancestors; it was 14% of matching time.

After changes 1–5, a profile of the wiki input shows: rule matching 35%, `set_mathml` 27% (cleanup and
canonicalization 15%, building the MathML string it returns 6%, parsing 4%), building a fresh XPath
context twice per equation 8%.

## The prefilter (change 5)

When a rule is loaded, the top level of its `match` XPath is scanned for a few shapes that are cheap to
test directly on the element: `count(*)=n`, `count(*)>=n`, `.='a' or .='b'`, `@attr...`, and
`IsBracketed(., 'a', 'b')`. They are only used when they are and'd together at the top level, so they must
hold for the rule to match. `find_match` checks them first and skips the XPath evaluation when one fails.
Anything unrecognized is ignored, so rule files need no changes and rule order is untouched.

It works on the XPath *text*, because sxd-xpath does not expose the parsed expression. A version worth
proposing upstream would read the parsed expression instead.

## Ideas that were not measured

- Reusing the XPath context (function table and preference variables) across calls: about 8% of time.
- Letting callers skip the MathML string `set_mathml` returns: about 6%.
- Rewriting the `turn-off` rule so it does not walk the ancestors of every element.
- More interpreter work in sxd-xpath (stop at the first match when only "is there any node" is needed,
  compare strings without copying them).
- Reordering rules: the cost is spread over many rules that each fail quickly, not a few slow ones tried
  too early, and order is part of a rule's meaning (first match wins), so there is little to gain.
- Compiling all of a tag's rules into one decision tree. The prefilter gets part of this benefit; after the
  interpreter fixes the remaining gain is at most the 35% that matching still costs.
