//! Runnable recipes shared by native Catalog and browser previews.
//! Each source file is a complete program; select/comparison and table/square
//! variants share their implementation rather than duplicating state or UI.
use std::sync::OnceLock;
pub mod button_sizes;
pub mod button_states;
pub mod button_variants;
pub mod select;
pub mod select_disabled;
pub mod table;

#[derive(Default)]
pub struct State {
    pub button_variants: button_variants::App,
    pub button_sizes: button_sizes::App,
    pub button_states: button_states::App,
    pub select: select::App,
    pub select_disabled: select_disabled::App,
    pub table: table::App,
}

pub const ALL: &[(&str, &str)] = &[
    ("button_variants", include_str!("button_variants.rs")),
    ("button_sizes", include_str!("button_sizes.rs")),
    ("button_states", include_str!("button_states.rs")),
    ("select", include_str!("select.rs")),
    ("select_disabled", include_str!("select_disabled.rs")),
    ("select_vs_combobox", include_str!("select.rs")),
    ("table", include_str!("table.rs")),
    ("table_square", include_str!("table.rs")),
];
static CODE: OnceLock<Vec<String>> = OnceLock::new();
/// Complete native program text. Hide only module-compilation cfg attributes,
/// not implementation lines. Formatting allocates once, never per redraw.
pub fn code(name: &str) -> &'static str {
    let index = ALL
        .iter()
        .position(|(id, _)| *id == name)
        .expect("known sample");
    CODE.get_or_init(|| {
        ALL.iter()
            .map(|(_, source)| {
                let mut code = source
                    .lines()
                    .filter(|line| !line.trim_end().ends_with("// catalog-only"))
                    .collect::<Vec<_>>()
                    .join("\n");
                code.push('\n');
                code
            })
            .collect()
    })[index]
        .as_str()
}
#[cfg(feature = "native")]
pub fn run(name: &str) -> Option<Result<(), Box<dyn std::error::Error>>> {
    match name.replace('-', "_").as_str() {
        "button_variants" => Some(button_variants::main()),
        "button_sizes" => Some(button_sizes::main()),
        "button_states" => Some(button_states::main()),
        "select" | "select_vs_combobox" => Some(select::main()),
        "select_disabled" => Some(select_disabled::main()),
        "table" | "table_square" => Some(table::main()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn copied_programs_hide_only_native_module_guards_and_cache_their_source() {
        assert_eq!(ALL.len(), 8);
        for (id, source) in ALL {
            for line in source
                .lines()
                .filter(|line| line.trim_end().ends_with("// catalog-only"))
            {
                assert_eq!(line.trim(), r#"#[cfg(feature = "native")] // catalog-only"#);
            }
            let shown = code(id);
            for needed in [
                "use volt_ui_kit::native::Application;",
                "use volt_ui_kit::ui::prelude::*;",
                "pub struct App",
                "pub fn view(",
                "pub fn main()",
                "Application::new(",
            ] {
                assert!(shown.contains(needed), "{id}: missing {needed}");
            }
            assert!(!shown.contains("catalog-only"));
            assert!(!shown.contains("#[cfg("));
            assert!(!shown.contains("super::"));
            assert!(!shown.contains("crate::"));
            assert!(
                std::ptr::eq(shown, code(id)),
                "source must be cached, not rebuilt per view"
            );
        }
        assert_eq!(code("table"), code("table_square"));
        assert_eq!(code("select"), code("select_vs_combobox"));
    }
}
