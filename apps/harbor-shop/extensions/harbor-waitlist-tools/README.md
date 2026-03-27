## Harbor Waitlist Tools

This is Harbor Shop's checked-in example of the bounded WASM extension path.

It is deliberately different from the linked Rust backend:

- it is runtime-installed rather than linked into the Harbor customer binary
- it stays inside explicit host grants
- it models a replaceable partner/marketplace add-on rather than Harbor Shop's own first-party
  checkout or webhook policy

The example package shape is captured in:

- `package.example.toml`
- `config-schema.example.toml`

What this example is showing:

- an admin widget for events waitlist pressure and exception handling
- a scheduled reconciliation job for partner waitlist sync
- capability-scoped host access instead of native runtime ownership

What this example is not:

- a linked customer plugin
- a sidecar service
- a deep transaction owner
- installed by default in the checked-in Harbor Shop app

If Harbor Shop wanted to activate this extension for a real customer deployment, the next step
would be:

1. build the actual `harbor-waitlist-tools.wasm` artifact
2. pin the final artifact checksum
3. add the corresponding extension installation entry to `app.toml`
4. keep the behavior bounded to host-approved grants

That is the chapter 80 path. If the feature instead grows into Harbor Shop-owned first-party
behavior or needs deeper transactional/rendering control, it should move out of this folder and
into linked Rust or a native module.
