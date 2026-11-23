# Proposal

## Why

Notes often explain a flow or an architecture. A Mermaid diagram in a `mermaid` code block shows that better than text, but mdBook shows the block as plain code.

## What Changes

- The `preprocess` command turns each `mermaid` code block into a `<pre class="mermaid">` HTML block when the renderer is `html`. Other renderers get the code block unchanged.
- Each chapter with at least one diagram gets one module script that loads Mermaid 11.4.1 from the jsDelivr CDN and draws the diagrams. The script picks Mermaid's dark theme when mdBook shows `coal`, `navy`, or `ayu`.
- Existing notebooks need no change to `book.toml`, because they already run the preprocessor.
- Out of scope:
  - drawing diagrams offline (no bundled copy of Mermaid);
  - redrawing diagrams when the reader changes the mdBook theme (a reload does it).

## Capabilities

### Modified Capabilities
- `book-rendering`: new requirement "Mermaid diagrams rendered".

## Impact

- New module `denote-mdbook-core/src/mermaid.rs`; `preprocess.rs` calls it.
- README documents the feature.
