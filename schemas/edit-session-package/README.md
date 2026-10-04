# EditSessionPackage 0.1 metadata schemas

`schema-bundle.json` is generated from the Rust types in
`crates/session-contract/src/model/`, using pinned Schemars 1.2.2. It contains six
independent JSON Schema 2020-12 documents keyed by `manifest`, `sources`, `time-maps`,
`visual-composition`, `audio-mix`, and `media-health`. Select the corresponding value
as the schema root; the bundle itself is not a schema for a package file.

Regenerate from the repository root:

```bash
cargo run --locked -p longcut-session-contract --bin session-inspect -- schema \
  > schemas/edit-session-package/schema-bundle.json
```

Schemas check document structure. The semantic validator additionally enforces
integer range, version support, references, stream/channel kinds, interval geometry,
coverage and safety policy. Schema acceptance alone does not imply metadata or
media conformance. See [contract usage](../../docs/session-contract.md).
