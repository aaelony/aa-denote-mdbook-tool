# aa-denote-mdbook-tool

 - [denote](https://protesilaos.com/emacs/denote) enables an efficient file-naming scheme for markdown files invoked within emacs.

 - [mdbook](https://rust-lang.github.io/mdBook/) is a tool to create books with markdown.

## Goal:

 - Create a Rust CLI tool that uses the file naming convention from `denote`, but uses the directory structure convention used by `mdbook`.

## Required Use Cases:

 - You want to quickly start taking notes and want the timestamp in `denote` convention.
 - You want to be able to render your `md` notes into an `mdbook` easily.
 - You want the tool to add the new note properly in the mdbook expected directory hierarchy, maintaining all linked references as `denote` does.  The linked urls must work even if they are relative paths.
 - You want to have a a folder for manual notes and a subfolder for AI Summarized notes from Zoom and another subfolder for AI Transcripts from Zoom.
 - You want a `png` folder where local `png` files needed are sourced from in the mdbook structure.
 - You want this to be easily searched via `ripgrep`.


 ### Example Use Cases

 It is Monday morning at 8am, a Zoom meeting is starting.  You will have back-to-back meetings for the next 8 hours.  You need manual notes, and want to preserve the AI note summaries and AI Transcripts labeled with filenames in the style of mdbook, such that later you can 1) Refresh the mdbook and review the rendered markdown, have subchapters for the full transcripts for full reference but not always looked at.  You also will use this as a knowledge base for LLM use with Claude and also possible interaction with GitHub, JIRA, and Aha! like tooling.

## Install

Requires Rust 1.85 or newer (edition 2024). [mdBook](https://rust-lang.github.io/mdBook/) is needed for `build`/`serve`, and [ripgrep](https://github.com/BurntSushi/ripgrep) for `search`.

```sh
cargo install --path .
cargo install mdbook ripgrep   # if not already installed
```

`book.toml` registers `aa-denote-mdbook-tool preprocess` as an mdBook preprocessor (it hides front matter in the rendered pages), so plain `mdbook build` also works once `aa-denote-mdbook-tool` is on your `PATH`.

## Notebook layout

```
work-notes/
  book.toml                     mdBook config + preprocessor
  .ignore                       keeps book/ out of ripgrep results
  src/
    SUMMARY.md                  generated between the aa-denote-mdbook-tool markers
    days/                       one generated page per day (rewritten by summary)
    README.md                   book introduction
    notes/                      manual notes
    notes/zoom-summaries/       Zoom AI summaries
    notes/zoom-transcripts/     Zoom AI transcripts
    png/ jpg/ svg/              images (created on first use, png/ by init)
    pdf/ sql/ json/             other attachments (created on first use)
  book/                         rendered output (mdbook build)
```

Every note is named the Denote way, `IDENTIFIER[==SIGNATURE][--title][__keywords].md`, for example `20261010T080000--weekly-sync__acme_meeting.md`, and starts with TOML front matter:

```toml
+++
title = "Weekly Sync"
date = 2026-10-10T08:00:00-07:00
tags = ["acme", "meeting"]
identifier = "20261010T080000"
kind = "manual"
+++
```

Imported notes also carry `parent` (the meeting's identifier) and `imported` (when the file was actually imported).

Attachments use the same naming with their own extension, for example `src/pdf/20261010T081500--vendor-contract__acme.pdf`. The supported types are `png`, `jpg`, `svg`, `pdf`, `sql`, and `json`. Images (`png`, `jpg`, `svg`) are referenced as `![Title](path)` so they render inline; the others as `[Title](path)` so they open or download from the book.

## Timestamps

A note's identifier records when the content happened, not when it was captured. The first of these that applies wins:

1. `--date "2026-10-10 08:30"` (also `YYYY-MM-DD`, `HH:MM` for today, or an identifier)
2. `--parent ID`: the parent meeting's time, plus one second per attachment
3. A Zoom file name such as `GMT20261010-150012_Recording.transcript.vtt` (UTC, converted to local time)
4. The current time

So transcripts imported at noon, or the next day, still sort and render right after their meeting.

## Commands

| Command | What it does |
|---|---|
| `init [DIR] [--title T]` | Create a notebook |
| `new [--title] [--keywords a,b] [--kind] [--parent ID] [--date] [--edit]` | Create a note; prints its path |
| `import FILE\|- --kind summary\|transcript [--parent ID] [--date]` | Import Zoom output (`.txt`/`.vtt` kept verbatim) |
| `attach FILE [--title] [--for ID]` (alias `image`) | Copy a png, jpg, svg, pdf, sql, or json file to `src/<ext>/`; prints `![..](..)` for images, `[..](..)` otherwise |
| `link ID [--from ID] [--append]` | Print (or append) a relative link |
| `links check` / `links fix` | Report or repair broken links; `fix` also converts `denote:ID` links and turns bare `http(s)://` URLs into links (as `import` does) |
| `links backlinks ID` | List notes linking to a note or attachment |
| `rename ID [--title] [--keywords] [--signature]` | Rename, keep the identifier, fix inbound links |
| `summary` / `build` / `serve [--open]` | Regenerate `SUMMARY.md`; build or serve the book |
| `search PATTERN [--keyword K] [--kind K] [-- RG_ARGS]` | ripgrep over the notes |

Every command except `init` finds the notebook from `--book DIR`, then `$AA_DENOTE_MDBOOK_TOOL_DIR`, then the nearest parent directory with a `book.toml`.

## Monday morning workflow

```sh
aa-denote-mdbook-tool init ~/work-notes --title "Work Notes"     # once
export AA_DENOTE_MDBOOK_TOOL_DIR=~/work-notes                    # once, e.g. in ~/.bashrc

# 8:00, 9:00, 10:00: one manual note per meeting, typed live
aa-denote-mdbook-tool new --title "Acme Weekly Sync" --keywords acme,meeting
aa-denote-mdbook-tool new --title "Roadmap Review" --keywords roadmap,meeting
aa-denote-mdbook-tool new --title "1:1 with Manager" --keywords oneonone

# Noon: attach the Zoom output to the 8:00 meeting (identifiers come from `new`)
aa-denote-mdbook-tool import ~/Downloads/summary.md --kind summary --parent 20261010T080000
aa-denote-mdbook-tool import ~/Downloads/GMT20261010-150012_Recording.transcript.vtt \
    --kind transcript --parent 20261010T080000

# Any time
aa-denote-mdbook-tool attach ~/Downloads/diagram.png --title "Arch Diagram" --for 20261010T080000
aa-denote-mdbook-tool attach ~/Downloads/contract.pdf --title "Vendor Contract" --keywords acme
aa-denote-mdbook-tool search budget --keyword acme --kind transcript -- -i
aa-denote-mdbook-tool links check
aa-denote-mdbook-tool build            # or: aa-denote-mdbook-tool serve --open
```

The rendered book lists each day as a linked entry, newest first, pointing to a generated page in `src/days/`. Each meeting is nested under its day, with its AI summary and transcript nested beneath it. The notes stay plain Markdown, so Claude and other tools can read them directly.

## Library

The logic lives in the `aa-denote-mdbook-tool-core` crate (folder `denote-mdbook-core/`) in this workspace. The `aa-denote-mdbook-tool` package builds the `aa-denote-mdbook-tool` command, a thin clap front end over it.
