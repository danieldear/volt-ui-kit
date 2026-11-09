//! Runnable code samples. Each file is a complete program: its own state, a
//! `view` and a `main`. The Catalog runs the same `view` as the live preview
//! and shows the file as the example's code, so the code you read is the code
//! that drew the preview.
//!
//! Run one on its own: `cargo run --features native --example samples -- button-variants`.
//!
//! Lines ending in `// catalog-only` let the browser build (which has no
//! native window) compile the file as a module. The code panel leaves them
//! out, because a copied file doesn't need them.
use std::borrow::Cow;

macro_rules! samples {
    ($($name:ident),* $(,)?) => {
        $(pub mod $name;)*

        /// Every sample's name and source file.
        pub const ALL: &[(&str, &str)] = &[
            $((stringify!($name), include_str!(concat!(stringify!($name), ".rs"))),)*
        ];

        /// The Catalog's copy of every sample's state.
        #[derive(Default)]
        pub struct State {
            $(pub $name: $name::App,)*
        }

        /// Run the named sample as its own app; `None` if there's no such sample.
        #[cfg(feature = "native")]
        pub fn run(name: &str) -> Option<Result<(), Box<dyn std::error::Error>>> {
            match name.replace('-', "_").as_str() {
                $(stringify!($name) => Some($name::main()),)*
                _ => None,
            }
        }
    };
}
samples!(
    button_variants,
    button_sizes,
    button_states,
    select,
    select_disabled,
    select_vs_combobox,
    table,
    table_square,
);

const CATALOG_ONLY: &str = "// catalog-only";

/// The named sample's source as the code panel shows it.
pub fn code(name: &str) -> Cow<'static, str> {
    let (_, source) = ALL
        .iter()
        .find(|(n, _)| *n == name)
        .unwrap_or_else(|| panic!("no sample named {name}"));
    source
        .lines()
        .filter(|line| !line.trim_end().ends_with(CATALOG_ONLY))
        .map(|line| format!("{line}\n"))
        .collect::<String>()
        .into()
}
