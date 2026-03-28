## Shoppr Waitlist Tools

This is Shoppr's checked-in example of the bounded WASM extension path.

It is deliberately different from the linked Rust backend:

- it is runtime-installed rather than linked into the Shoppr customer binary
- it stays inside explicit host grants
- it models a replaceable partner/marketplace add-on rather than Shoppr's own first-party
  checkout or webhook policy

The runtime-installed example is captured in:

- `package.toml`
- `shoppr-waitlist-tools.wat`

What this example is showing:

- a real `[[extensions]]` installation entry in Shoppr's `app.toml`
- a bounded render hook that runs on the checked-in CMS home page
- capability-scoped host access instead of native runtime ownership

What this example is not:

- a linked customer plugin
- a sidecar service
- a deep transaction owner

Shoppr bootstrap compiles the checked-in WAT source into the pinned `.wasm` artifact before the
runtime plan is built, so this folder now demonstrates a real installed extension path rather than
package-shape only.

That is the chapter 80 path. If the feature instead grows into Shoppr-owned first-party
behavior or needs deeper transactional/rendering control, it should move out of this folder and
into linked Rust or a native module.
