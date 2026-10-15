//! Explicit, offline documentation export. No window, workers or desktop calls.
use super::{pages, App, Page, PAGES};
use serde_json::json;
use std::path::Path;
use volt_ui_kit::{
    declarative::{Bindings, Runtime},
    native::RasterPainter,
    ui::prelude::*,
    Viewport,
};

/// Export source-backed examples and settled software previews into a new directory.
/// Existing output is never overwritten or deleted.
pub fn write(output: &Path) -> Result<(), Box<dyn std::error::Error>> {
    std::fs::create_dir(output)?;
    let mut app = App::new();
    app.glass = false;
    app.theme = 2;
    app.mode = ThemeMode::Light;
    let theme = super::theme(&app, ThemeMode::Light);
    let mut entries = vec![];
    for page in PAGES {
        app.page = page;
        app.dialog_open = page == Page::Dialog;
        let doc = pages::doc(page, &app);
        let mut sections = vec![];
        for section in doc.sections {
            let filename = format!("{}-{}.png", page.slug(), section.id);
            if !filename
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c))
            {
                return Err("invalid documentation image name".into());
            }
            let demo = if page == Page::Dialog {
                let mut layers = vec![section.demo];
                layers.extend(pages::dialogs(&app));
                zstack(layers).width(936.).height(536.).into()
            } else {
                section.demo
            };
            let element: Element<App> = vstack![demo]
                .align(Align::Center)
                .padding(32., Edge::All)
                .into();
            let (document, handlers) = element.build()?.into_parts();
            let bindings = Bindings::default();
            let mut runtime = Runtime::new(document, &handlers, &bindings)?;
            runtime.set_reduced_motion(true);
            // Measure at a generous height, then remove unused space. Tall,
            // scrollable demos remain viewport captures rather than fake full content.
            let measure = Viewport::new(1000., 1600., 1.).unwrap();
            let mut painter = RasterPainter::new(measure);
            runtime.layout(measure, &bindings, &mut painter)?;
            let extent = runtime
                .boxes()
                .iter()
                .skip(1)
                .map(|b| {
                    let [_, y, _, h] = b.bounds.bounds();
                    y + h
                })
                .fold(160_f32, f32::max);
            let height = if page == Page::Dialog {
                600
            } else {
                (extent + 32.).clamp(200., 1600.).ceil() as u32
            };
            let viewport = Viewport::new(1000., height as f32, 1.).unwrap();
            let mut painter = RasterPainter::new(viewport);
            runtime.layout(viewport, &bindings, &mut painter)?;
            runtime.settle_animations();
            let batches = runtime.paint_ui(&mut painter, &theme);
            painter
                .rasterize(&batches, theme.legacy_theme().background)?
                .save_png(output.join(&filename))?;
            sections.push(json!({"id":section.id,"title":section.title,
                "note":section.note,"code":section.code,"image":filename,
                "width":1000,"height":height}));
        }
        entries.push(json!({"slug":page.slug(),"title":page.title(),
            "group":page.group(),"summary":page.summary(),
            "sections":sections,"api":doc.api}));
        println!("Exported {}", page.title());
    }
    std::fs::write(
        output.join("catalog.json"),
        serde_json::to_vec_pretty(&entries)?,
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn published_metadata_matches_every_catalog_page_and_section() {
        let exported: serde_json::Value =
            serde_json::from_str(include_str!("../../docs/assets/catalog-data.json")).unwrap();
        let entries = exported.as_array().unwrap();
        assert_eq!(entries.len(), PAGES.len());
        let app = App::new();
        for (entry, page) in entries.iter().zip(PAGES) {
            assert_eq!(entry["slug"], page.slug());
            assert_eq!(entry["title"], page.title());
            assert_eq!(entry["summary"], page.summary());
            let doc = pages::doc(page, &app);
            assert_eq!(entry["api"], serde_json::to_value(doc.api).unwrap());
            let sections = entry["sections"].as_array().unwrap();
            assert_eq!(sections.len(), doc.sections.len());
            for (data, section) in sections.iter().zip(doc.sections) {
                assert_eq!(data["id"], section.id);
                assert_eq!(data["code"], section.code);
                assert_eq!(data["title"], section.title);
                assert_eq!(data["note"], section.note);
            }
        }
    }

    #[test]
    fn export_refuses_existing_directory_without_modifying_it() {
        let directory = std::env::temp_dir().join(format!(
            "volt-doc-export-test-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        std::fs::create_dir(&directory).unwrap();
        let marker = directory.join("keep.txt");
        std::fs::write(&marker, "keep").unwrap();
        assert!(write(&directory).is_err());
        assert_eq!(std::fs::read_to_string(&marker).unwrap(), "keep");
        std::fs::remove_file(marker).unwrap();
        std::fs::remove_dir(directory).unwrap();
    }
}
