//! Core logic for aa-denote-mdbook-tool: Denote file names in an mdBook directory
//! layout. The `aa-denote-mdbook-tool` binary is a thin clap front end over this crate.

pub mod capture;
pub mod external;
pub mod frontmatter;
pub mod links;
pub mod naming;
pub mod notebook;
pub mod preprocess;
pub mod search;
pub mod summary;
