#!/usr/bin/env just --justfile

main_crate := 'aa-denote-mdbook-tool'
# How to call the current just executable. Note that just_executable() may have `\` in Windows paths, so we need to quote it.
just := quote(just_executable())
# cargo-binstall needs a workaround due to caching when used in CI
binstall_args := if env('CI', '') != '' {'--no-confirm --no-track --disable-telemetry'} else {''}
# location of the coverage output, used by CI
coverage_lcov := 'target/llvm-cov/lcov.info'
# Test targets: library unit tests, the binary, and the CLI integration tests in tests/.
test_targets := '--lib --bins --tests'
# `cargo llvm-cov report` has no --workspace flag, so name every workspace package
coverage_packages := '-p aa-denote-mdbook-tool -p aa-denote-mdbook-tool-core'
# Synthetic notebook used by the profiling recipes (see `profile-notebook`)
profile_notebook := justfile_directory() / 'target/profile-notebook'
# Profiling build: release + debug symbols + frame pointers (perf `--call-graph fp` attributes ~100%
# of samples to our functions, vs. many `[unknown]` frames and a ~600 MB perf.data with DWARF unwinding).
# Its own target dir, so switching between this and normal release builds does not force rebuilds.
profiling_target := justfile_directory() / 'target/profiling'
profile_env := 'CARGO_PROFILE_RELEASE_DEBUG=true RUSTFLAGS="-C force-frame-pointers=yes" CARGO_TARGET_DIR=' + quote(profiling_target)
profile_bin := profiling_target / 'release/' + main_crate
# perf sampling frequency in Hz for flamegraphs (perf's default of ~1000 gives too few samples for sub-second runs)
flamegraph_freq := '20000'

# if running in CI, treat warnings as errors by setting CARGO_BUILD_WARNINGS to 'deny' unless it is already set
# Use `CI=true just ci-test` to run the same tests as in GitHub CI.
# Use `just env-info` to see the current value of CARGO_BUILD_WARNINGS
ci_mode := if env('CI', '') != '' {'1'} else {''}
export CARGO_BUILD_WARNINGS := env('CARGO_BUILD_WARNINGS', if ci_mode == '1' {'deny'} else {'warn'})
export RUST_BACKTRACE := env('RUST_BACKTRACE', if ci_mode == '1' {'1'} else {'0'})

@_default:
    {{just}} --list

# Build the project
build:
    cargo build --workspace --all-features --all-targets

build_release:
	cargo build --workspace --all-features --all-targets --release

deploy: build_release
	cp -p target/release/aa-denote-mdbook-tool ~/.local/bin
	echo "copied to ~/.local/bin/aa-ddenote-mdbook-tool"

# Quick compile without building a binary
check:
    cargo check --workspace --all-features --all-targets

# Generate LCOV coverage report for CI to upload to codecov.io
ci-coverage: env-info && \
        (_coverage '--lcov' '--output-path' quote(coverage_lcov))
    rm -rf {{quote(parent_directory(coverage_lcov))}}
    mkdir -p {{quote(parent_directory(coverage_lcov))}}

# Run all tests as expected by CI
ci-test: env-info test-fmt clippy test test-doc && assert-git-is-clean

# Compile default features with minimal dependencies on the configured MSRV
ci-test-msrv:
    {{just}} ci_mode=0 env-info _check-msrv-default
    {{just}} assert-git-is-clean

# Set toolchain and run ci-test-msrv
ci-test-msrv-with-toolchain:
    RUSTUP_TOOLCHAIN="$({{just}} get-msrv)" {{just}} ci-test-msrv

# Clean all build artifacts (Cargo.lock is kept: this is a binary crate)
clean:
    cargo clean

# Run cargo clippy to lint the code
clippy *args:
    cargo clippy --workspace --all-features --all-targets {{args}}

# Generate and open the HTML coverage report
coverage:  (_coverage '--open')

# Print a per-file coverage table in the terminal
coverage-summary:  (_coverage '--summary-only')

# Clean, collect, and aggregate coverage using the requested report arguments.
# The integration tests run the instrumented binary as a child process, so CLI code is covered too.
_coverage *report_args:  (cargo-install 'cargo-llvm-cov')
    cargo llvm-cov clean --workspace
    cargo llvm-cov --no-report --workspace --all-features {{test_targets}}
    if {{just}} _has-nightly; then cargo +nightly llvm-cov --no-report --doctests --workspace --all-features; fi
    cargo llvm-cov report {{coverage_packages}} --include-build-script {{report_args}}

# Build and open code documentation
docs *args='--open':
    DOCS_RS=1 cargo doc --no-deps {{args}} --workspace --all-features

# Print environment info
env-info:
    @echo "Running for '{{main_crate}}' crate {{if ci_mode == '1' {'in CI mode'} else {'in dev mode'} }} on {{os()}} / {{arch()}}"
    @echo "PWD {{justfile_directory()}}"
    {{just}} --version
    rustc --version
    cargo --version
    rustup --version
    @echo "CARGO_BUILD_WARNINGS='$CARGO_BUILD_WARNINGS'"
    @echo "RUST_BACKTRACE='$RUST_BACKTRACE'"

# Reformat all code `cargo fmt`. If nightly is available, use it for better results
fmt:
    #!/usr/bin/env bash
    set -euo pipefail
    if {{just}} _has-nightly && rustup component list --toolchain nightly --installed 2>/dev/null | grep -q rustfmt; then
        echo 'Reformatting Rust code using nightly Rust fmt to sort imports'
        cargo +nightly fmt --all -- --config imports_granularity=Module,group_imports=StdExternalCrate
    else
        echo 'Reformatting Rust with the stable cargo fmt.  Install nightly with `rustup install nightly` for better results'
        cargo fmt --all
    fi

# Reformat all Cargo.toml files using cargo-sort
fmt-toml *args:  (cargo-install 'cargo-sort')
    cargo sort --workspace --grouped {{args}}

# Get a package field from the metadata
get-crate-field field package=main_crate:  (assert-cmd 'jq')
    @cargo metadata --no-deps --format-version 1 | jq -e -r '.packages | map(select(.name == "{{package}}")) | first | .{{field}} // error("Field \"{{field}}\" is missing in Cargo.toml for package {{package}}")'

# Get the minimum supported Rust version (MSRV) for the crate
get-msrv package=main_crate:  (get-crate-field 'rust_version' package)

# Find the minimum supported Rust version (MSRV), update Cargo.toml, and test minimal dependencies
msrv:  (cargo-install 'cargo-msrv')
    cargo msrv find --write-msrv --ignore-lockfile -- {{just}} _check-msrv-default

# Compile the crate's default features using a dynamically generated minimal Cargo.lock
_check-msrv-default:  (cargo-install 'cargo-minimal-versions') (cargo-install 'cargo-hack')
    #!/usr/bin/env bash
    set -euo pipefail
    # cargo-msrv probes with rustup, but nested cargo subcommands may otherwise
    # fall back to the default Cargo and emit flags unsupported by the candidate rustc.
    toolchain="$(rustc --version | cut -d' ' -f2)"
    export RUSTUP_TOOLCHAIN="$toolchain"
    export CARGO="$(rustup which --toolchain "$toolchain" cargo)"
    cargo minimal-versions check --direct --package {{main_crate}}

# Mutation-test every workspace crate, running all workspace tests (the CLI integration tests exercise the core). Extra args go to cargo-mutants, e.g. `just mutants --file src/cli.rs`
mutants *args:  (cargo-install 'cargo-mutants')
    cargo mutants --workspace --test-workspace=true {{args}}

# Each meeting gets an attached AI summary and transcript (`lines` lines), plus cross-links.
# Generate a synthetic notebook in target/profile-notebook for profiling (regenerates from scratch)
profile-notebook meetings='2000' lines='200':
    #!/usr/bin/env bash
    set -euo pipefail
    {{profile_env}} cargo build --release --quiet
    bin={{quote(profile_bin)}}
    nb="{{profile_notebook}}"
    rm -rf "$nb"
    "$bin" init "$nb" --title "Profile Notebook" 2>/dev/null >/dev/null
    notes="$nb/src/notes"
    kws=(acme beta gamma delta)
    line="Speaker: we discussed the budget, the roadmap, and next steps for the acme rollout."
    body="$(for _ in $(seq {{lines}}); do echo "$line"; done)"
    prev=""
    day_str=""
    for ((i = 0; i < {{meetings}}; i++)); do
        # Eight meetings per working day, 08:00 to 15:00.
        if (( i % 8 == 0 )); then
            day_str="$(date -d "2026-01-01 + $((i / 8)) days" +%Y%m%d)"
            iso_day="${day_str:0:4}-${day_str:4:2}-${day_str:6:2}"
        fi
        hh="$(printf '%02d' $((8 + i % 8)))"
        kw="${kws[$((i % 4))]}"
        id="${day_str}T${hh}0000"
        m="${id}--meeting-${i}__${kw}_meeting.md"
        s="${day_str}T${hh}0001--meeting-${i}-ai-summary__${kw}_meeting_zoomsummary.md"
        t="${day_str}T${hh}0002--meeting-${i}-transcript__${kw}_meeting_zoomtranscript.md"
        fm() { # title id kind sec [parent]
            printf '+++\ntitle = "%s"\ndate = %sT%s:00:%s-07:00\ntags = ["%s", "meeting"]\nidentifier = "%s"\nkind = "%s"\n' \
                "$1" "$iso_day" "$hh" "$4" "$kw" "$2" "$3"
            [[ -n "${5:-}" ]] && printf 'parent = "%s"\n' "$5"
            printf '+++\n\n# %s\n\n' "$1"
        }
        {
            fm "Meeting $i" "$id" manual 00
            [[ -n "$prev" ]] && printf 'Follows [previous meeting](%s).\n\n' "$prev"
            printf '## Attachments\n\n- [Summary](zoom-summaries/%s)\n- [Transcript](zoom-transcripts/%s)\n' "$s" "$t"
        } > "$notes/$m"
        {
            fm "Meeting $i AI Summary" "${day_str}T${hh}0001" summary 01 "$id"
            printf 'Back to [the meeting](../%s). Budget approved; see [transcript](../zoom-transcripts/%s).\n' "$m" "$t"
        } > "$notes/zoom-summaries/$s"
        {
            fm "Meeting $i Transcript" "${day_str}T${hh}0002" transcript 02 "$id"
            printf '```text\n%s\n```\n' "$body"
        } > "$notes/zoom-transcripts/$t"
        prev="$m"
    done
    "$bin" --book "$nb" summary 2>/dev/null
    echo "Generated $(find "$nb/src/notes" -name '*.md' | wc -l) notes in $nb ($(du -sh "$nb/src" | cut -f1))"

# Generate the profiling notebook only if it does not exist yet
[private]
_ensure-profile-notebook:
    @[ -f "{{profile_notebook}}/book.toml" ] || {{just}} profile-notebook

# Needs perf; if it is refused, run `sudo sysctl kernel.perf_event_paranoid=1`.
# Writes target/flamegraphs/<command>.svg. Examples: `just flamegraph links check`, `just flamegraph search budget --keyword acme`
# Flamegraph one CLI command against the profiling notebook, e.g. `just flamegraph summary`
flamegraph *cmd='summary':  (cargo-install 'cargo-flamegraph' 'flamegraph') _ensure-profile-notebook
    #!/usr/bin/env bash
    set -euo pipefail
    out="{{justfile_directory()}}/target/flamegraphs"
    mkdir -p "$out"
    # Run perf from the output dir so perf.data never lands in the repo root; remove it afterwards.
    cd "$out"
    trap 'rm -f perf.data perf.data.old' EXIT
    name="$(echo '{{cmd}}' | tr -c 'A-Za-z0-9\n' '-' | tr -s '-' | sed 's/-$//')"
    {{profile_env}} cargo flamegraph --manifest-path {{quote(justfile_directory() / 'Cargo.toml')}} \
        --release --bin {{main_crate}} --output "$name.svg" \
        --cmd "record -F {{flamegraph_freq}} --call-graph fp -g" \
        -- --book "{{profile_notebook}}" {{cmd}} > /dev/null
    echo "Wrote target/flamegraphs/$name.svg"

# Flamegraph the read-heavy commands: summary, links check, backlinks, and a filtered search
flamegraph-all: \
        (flamegraph 'summary') \
        (flamegraph 'links' 'check') \
        (flamegraph 'links' 'backlinks' '20260101T080000') \
        (flamegraph 'search' 'budget' '--keyword' 'acme' '--kind' 'transcript')

# Wall-clock time of one CLI command against the profiling notebook, e.g. `just profile-time links check`
profile-time *cmd='summary':  _ensure-profile-notebook
    #!/usr/bin/env bash
    set -euo pipefail
    {{profile_env}} cargo build --release --quiet
    TIMEFORMAT='%R s elapsed, %U s user, %S s system'
    time {{quote(profile_bin)}} --book "{{profile_notebook}}" {{cmd}} > /dev/null 2>&1

# Run cargo-release
release *args='':  (cargo-install 'release-plz')
    release-plz {{args}}

# Check semver compatibility with prior published version. Install it with `cargo install cargo-semver-checks`
semver *args:  (cargo-install 'cargo-semver-checks')
    cargo semver-checks --all-features {{args}}

# Run all tests: unit tests in aa-denote-mdbook-tool-core and the CLI integration tests
test:
    cargo test --workspace --all-features {{test_targets}}
    cargo test --doc --workspace --all-features

# Test documentation generation
test-doc:  (docs '')

# Test code formatting
test-fmt: && (fmt-toml '--check' '--check-format')
    cargo fmt --all -- --check

# Find unused dependencies. Uses `cargo-udeps`
udeps:  (cargo-install 'cargo-udeps')
    cargo +nightly udeps --workspace --all-features --all-targets

# Update all dependencies, including breaking changes. Requires nightly toolchain (install with `rustup install nightly`)
update:
    cargo +nightly -Z unstable-options update --breaking
    cargo update

# Succeeds only if `cargo +nightly` works (stricter than grepping `rustup toolchain list`, which also matches dated nightlies)
[private]
_has-nightly:
    @rustup run nightly rustc --version > /dev/null 2>&1

# Ensure that a certain command is available
[private]
assert-cmd command:
    @if ! type {{command}} > /dev/null; then \
        echo "Command '{{command}}' could not be found. Please make sure it has been installed on your computer." ;\
        exit 1 ;\
    fi

# Make sure the git repo has no uncommitted changes. Fails if CI envvar is set.
[private]
assert-git-is-clean:
    #!/usr/bin/env bash
    set -euo pipefail
    if [ -n "$(git status --porcelain --untracked-files=all)" ]; then
        >&2 echo "::error::git repo is not clean. Make sure compilation and tests artifacts are in the .gitignore, and no repo files are modified."
        if [[ "{{ci_mode}}" == "1" ]]; then
            >&2 echo "::group::git status"
            git status
            >&2 echo "::endgroup::"
            >&2 echo "::group::git diff (tracked changes)"
            git add . --intent-to-add
            git --no-pager diff
            >&2 echo "::endgroup::"
            exit 1
        else
            >&2 echo "git repo is not clean, but not failing because CI mode is not enabled."
        fi
    fi

# Check if a certain Cargo command is installed, and install it if needed
[private]
cargo-install $COMMAND $INSTALL_CMD='' *args='':
    #!/usr/bin/env bash
    set -euo pipefail
    unset CARGO_BUILD_WARNINGS
    if ! command -v $COMMAND > /dev/null; then
        echo "$COMMAND could not be found. Installing..."
        if ! command -v cargo-binstall > /dev/null; then
            set -x
            cargo install ${INSTALL_CMD:-$COMMAND} --locked {{args}}
            { set +x; } 2>/dev/null
        else
            set -x
            cargo binstall ${INSTALL_CMD:-$COMMAND} {{binstall_args}} --locked
            { set +x; } 2>/dev/null
        fi
    fi

# CRAP score (complexity x missing coverage) for every function in the workspace; fails if any is above 30
crap *args:  (cargo-install 'cargo-llvm-cov') (cargo-install 'cargo-crap')
    mkdir -p {{quote(parent_directory(coverage_lcov))}}
    cargo llvm-cov --workspace --all-features {{test_targets}} --lcov --output-path {{quote(coverage_lcov)}}
    cargo crap --workspace --lcov {{quote(coverage_lcov)}} --threshold 30 --fail-above {{args}}
