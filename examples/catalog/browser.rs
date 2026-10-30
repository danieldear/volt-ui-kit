//! The browser uses the Catalog's original builders and callbacks. Only native
//! OS integrations are replaced by explicit capability messages, not simulations.
use super::{
    pages::{self, Doc, Section},
    App, Page, PAGES,
};
use volt_ui_kit::ui::prelude::*;

pub fn app(page: &str, section: &str, dark: bool, accent: usize) -> Result<App, String> {
    if accent >= super::THEMES.len() {
        return Err("unknown accent".into());
    }
    let page = PAGES
        .into_iter()
        .find(|p| p.slug() == page)
        .ok_or("unknown Catalog page")?;
    let mut app = App::new();
    app.page = page;
    app.web_section = section.into();
    app.theme = accent;
    app.mode = if dark {
        ThemeMode::Dark
    } else {
        ThemeMode::Light
    };
    let special = page == Page::Introduction && matches!(section, "counter" | "plan");
    if !special
        && !section.is_empty()
        && !pages::doc(page, &app)
            .sections
            .iter()
            .any(|s| s.id == section)
    {
        return Err("unknown Catalog section".into());
    }
    Ok(app)
}

pub fn view(app: &App) -> Element<App> {
    let demo: Element<App> = match (app.page, app.web_section.as_str()) {
        (Page::Introduction, "counter") => super::landing::counter_sample(app),
        (Page::Introduction, "plan") => super::landing::plan_sample(app),
        (Page::Introduction, _) => super::landing::landing_hero(app),
        _ => {
            let sections = pages::doc(app.page, app).sections;
            let index = sections
                .iter()
                .position(|s| app.web_section.is_empty() || s.id == app.web_section)
                .unwrap_or(0);
            sections
                .into_iter()
                .nth(index)
                .map(|s| s.demo)
                .unwrap_or_else(|| text("This section is not available.").into())
        }
    };
    let mut layers = vec![vstack![demo]
        .align(Align::Center)
        .padding(32., Edge::All)
        .fill_width()
        .into()];
    if app.page == Page::Dialog {
        layers.extend(pages::dialogs(app));
    }
    if let Some((_, message)) = &app.toast {
        layers.push(
            vstack![
                spacer(),
                panel![text(message).font_size(12.)]
                    .padding(12., Edge::All)
                    .background(ColorToken::Base300)
                    .rounded(8.)
            ]
            .padding(12., Edge::All)
            .into(),
        );
    }
    zstack(layers)
        .key("web-catalog-root")
        .align(Align::Stretch)
        .font_family("Inter")
        .fill_width()
        .fill_height()
        .scroll(Scroll::Both)
        .background(ColorToken::Base100)
        .into()
}

pub fn native_doc(page: Page) -> Doc {
    let entries: &[(&str, &str)] = match page {
        Page::ApplicationLifecycle => &[("application-document", "Application lifecycle")],
        Page::Documents => &[("document-files", "Native file workflows")],
        Page::DesktopServices => &[
            ("desktop-notifications", "OS notifications"),
            ("desktop-tray", "Native tray commands"),
        ],
        _ => &[],
    };
    Doc {sections:entries.iter().map(|&(id,title)|Section {
        id,title,note:"Native OS integration: unavailable in a browser sandbox.",code:"Run the native Catalog for this operating-system integration.",
        demo:vstack![badge("Native only").background(ColorToken::Warning.alpha(14)).color(ColorToken::Warning),text(title).font_size(22.).font_weight(600),text("This is an operating-system service, not a browser component. It requires the native Catalog; the web preview does not request permissions, send notifications, access files or create a tray icon.").font_size(14.).color(ColorToken::Muted)].max_width(540.).gap(14.,Axis::Vertical).padding(24.,Edge::All).border_width(1.).border_color(ColorToken::Border).rounded(12.).into(),
    }).collect(),api:vec![]}
}

#[cfg(test)]
mod tests {
    use super::*;
    use volt_ui_kit_web::{WebEvent, WebView};
    #[test]
    fn every_published_catalog_section_uses_shared_builders_and_settles() {
        let app = App::new();
        let mut count = 0;
        for page in PAGES {
            let sections: Vec<_> = pages::doc(page, &app)
                .sections
                .into_iter()
                .map(|s| s.id)
                .collect();
            if sections.is_empty() {
                continue;
            }
            let state = super::app(&page.slug(), sections[0], false, 0).unwrap();
            let mut host =
                WebView::new(state, view).unwrap_or_else(|e| panic!("{}: {e}", page.slug()));
            for id in sections {
                host.update(|a| a.web_section = id.into())
                    .unwrap_or_else(|e| panic!("{}/{id}: {e}", page.slug()));
                assert!(
                    !host.snapshot()["nodes"].as_array().unwrap().is_empty(),
                    "{}/{id}",
                    page.slug()
                );
                count += 1;
            }
        }
        assert_eq!(count, 123);
    }
    #[test]
    fn landing_counter_and_appearance_reuse_original_callback_state() {
        let mut host = WebView::new(
            super::app("introduction", "counter", false, 2).unwrap(),
            view,
        )
        .unwrap();
        host.dispatch(WebEvent::Activate {
            id: "landing-counter".into(),
        })
        .unwrap();
        assert_eq!(host.context().counter, 1);
        host.release_draw_cache();
        host.update(|a| {
            a.theme = 4;
            a.mode = ThemeMode::Dark;
        })
        .unwrap();
        assert_eq!(host.context().counter, 1);
        assert!(super::app("button", "not-a-section", false, 0).is_err());
        assert!(super::app("not-a-page", "", false, 0).is_err());
    }
}
