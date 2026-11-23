## ADDED Requirements

### Requirement: Mermaid diagrams rendered
When mdBook runs the `aa-denote-mdbook-tool` preprocessor for the `html` renderer, the tool SHALL replace each fenced code block whose language is `mermaid` with a `<pre class="mermaid">` block that holds the HTML-escaped diagram source. It SHALL add one script to each chapter that has a diagram, and that script SHALL load Mermaid and draw the diagrams. It SHALL leave the source files, code blocks nested inside other code blocks, and the output for other renderers unchanged.

#### Scenario: Note with a diagram
- **WHEN** a note contains a ```` ```mermaid ```` block with `A-->B` and the book is built
- **THEN** the note's HTML page contains `<pre class="mermaid">` with `A--&gt;B` and a script that loads Mermaid
- **AND** the source Markdown file still contains the ```` ```mermaid ```` block

#### Scenario: Page without a diagram
- **WHEN** a page has no `mermaid` code block
- **THEN** its HTML page does not load Mermaid

#### Scenario: Other renderer
- **WHEN** the preprocessor runs for a renderer other than `html`
- **THEN** the `mermaid` code block is passed through unchanged
