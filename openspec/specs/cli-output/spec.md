# cli-output Specification

## Purpose

Defines what the CLI writes to standard output versus standard error, so its output can be piped and parsed while still identifying the running version.

## Requirements

### Requirement: Startup banner
Every command except the hidden `preprocess` SHALL print one line `<package name> <package version>`, taken from the build, to standard error before running. Standard output SHALL never contain the banner.

#### Scenario: Banner on a normal command
- **WHEN** the user runs `aa-denote-mdbook-tool new --title "X"` with version 0.1.0
- **THEN** standard error begins with `aa-denote-mdbook-tool 0.1.0`
- **AND** standard output is exactly the new note's path

#### Scenario: Preprocessor stays silent
- **WHEN** mdBook runs `aa-denote-mdbook-tool preprocess supports html`
- **THEN** nothing is written to standard error

#### Scenario: Version flag
- **WHEN** the user runs `aa-denote-mdbook-tool --version`
- **THEN** standard output is `aa-denote-mdbook-tool 0.1.0` and no separate banner is printed
