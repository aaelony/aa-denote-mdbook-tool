//! aa-denote-mdbook-tool: Denote file names in an mdBook directory layout.

mod cli;

fn main() -> std::process::ExitCode {
    cli::main()
}
