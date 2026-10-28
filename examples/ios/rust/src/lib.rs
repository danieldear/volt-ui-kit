//! Field Notes: the screens, navigation, validation and state are ordinary Rust.
use volt_ui_kit::{
    mobile::MobileView,
    ui::{prelude::*, Button},
};

#[derive(Clone, Copy, Debug, PartialEq)]
enum Screen {
    Notes,
    Detail(usize),
    Profile,
}
pub struct App {
    screen: Screen,
    return_to: Screen,
    name: String,
    email: String,
    draft_name: String,
    draft_email: String,
    message: String,
    font: String,
    dark: bool,
}
impl Default for App {
    fn default() -> Self {
        Self {
            screen: Screen::Notes,
            return_to: Screen::Notes,
            name: "Daniel".into(),
            email: "daniel@example.com".into(),
            draft_name: String::new(),
            draft_email: String::new(),
            message: String::new(),
            font: "sans-serif".into(),
            dark: false,
        }
    }
}
impl App {
    fn edit(&mut self) {
        self.return_to = self.screen;
        self.draft_name = self.name.clone();
        self.draft_email = self.email.clone();
        self.message.clear();
        self.screen = Screen::Profile;
    }
    fn save(&mut self) {
        let name = self.draft_name.trim();
        let email = self.draft_email.trim();
        // Intentionally modest demo validation, not an email-verification claim.
        if name.is_empty() || name.chars().count() > 60 {
            self.message = "Enter a name of 1–60 characters.".into();
        } else if !email.contains('@')
            || !email.rsplit('@').next().is_some_and(|v| v.contains('.'))
            || email.chars().any(char::is_whitespace)
            || email.len() > 254
        {
            self.message = "Enter an email like name@example.com.".into();
        } else {
            self.name = name.into();
            self.email = email.into();
            self.message = "Profile saved for this session.".into();
            self.screen = self.return_to;
        }
    }
}
const TITLES: [&str; 6] = [
    "Make room for good ideas",
    "A walk without a destination",
    "Objects worth keeping",
    "Design for the everyday",
    "A little less, a little better",
    "Notes from the coast",
];
fn title(i: usize) -> String {
    format!("{} · {:02}", TITLES[i % 6], i + 1)
}
fn nav(label: &str, key: &str) -> Button<App> {
    button(label)
        .key(key)
        .height(44.)
        .rounded(12.)
        .variant(ButtonVariant::Ghost)
        .font_size(15.)
        .font_weight(600)
}
fn content(app: &App) -> Element<App> {
    let muted = if app.dark { "#A8B6B3" } else { "#657671" };
    let card = if app.dark { "#20312D" } else { "#FFFFFF" };
    match app.screen {
        Screen::Notes => vstack![
            hstack![text("FIELD NOTES").font_size(12.).font_weight(700).color(muted), spacer(), nav("Profile", "profile").on_click(App::edit)].align(Align::Center),
            text("A place for\ngood thoughts.").font_size(36.).font_weight(700),
            text(format!("Welcome back, {}. Take your time.", app.name)).font_size(16.).color(muted),
            text(&app.message).key("status").font_size(13.).color(muted),
            hstack![text("YOUR NOTEBOOK").font_size(11.).font_weight(700).color(muted), spacer(), text("24 entries").font_size(12.).color(muted)].height(30.).align(Align::Center),
            vstack((0..24).map(|i| vstack![
                text(format!("{:02}  /  OBSERVATIONS", i+1)).font_size(10.).font_weight(700).color(muted),
                button(title(i)).key(format!("note-{i}")).accessible_label(format!("Open note {}", i+1)).variant(ButtonVariant::Ghost).font_size(18.).font_weight(600).height(52.).on_click(move |a: &mut App| { a.screen = Screen::Detail(i); a.message.clear(); }),
                text("Small details. A different perspective.").font_size(13.).color(muted),
            ].gap(8., Axis::Vertical).padding(18., Edge::All).background(card).rounded(18.).into()).collect::<Vec<_>>())
                .key("notes-list").flex(1.).gap(12., Axis::Vertical).scroll(Scroll::Vertical),
        ].gap(12., Axis::Vertical).into(),
        Screen::Detail(i) => vstack![
            hstack![nav("‹ Notes", "back").on_click(|a: &mut App| a.screen = Screen::Notes), spacer(), nav("Profile", "profile").on_click(App::edit)],
            vstack![
                text(format!("OBSERVATION / {:02}", i+1)).font_size(11.).font_weight(700).color(muted),
                text(TITLES[i%6]).font_size(34.).font_weight(700),
                text("The most useful ideas often begin with noticing something ordinary.").font_size(20.).color(muted),
                panel![text("LOOK CLOSER").font_size(11.).font_weight(700), text("Leave enough space for an unexpected thought.").font_size(24.).font_weight(600)]
                    .padding(24., Edge::All).gap(16., Axis::Vertical).background(if app.dark { "#184A3D" } else { "#DDECE4" }).rounded(22.),
                text("This small notebook is rendered by Volt in Rust. The same layout, text and callback system powers the desktop toolkit.").font_size(17.),
                text("Scroll, navigate away, edit your profile, then come back. The state stays in Rust; the platform supplies the window and keyboard.").font_size(17.).color(muted),
                text(&app.message).key("status").font_size(14.).color(muted),
            ].key("detail-scroll").flex(1.).scroll(Scroll::Vertical).gap(26., Axis::Vertical),
        ].gap(20., Axis::Vertical).into(),
        Screen::Profile => vstack![
            hstack![nav("Cancel", "cancel").on_click(|a: &mut App| { a.screen = a.return_to; a.message.clear(); }), spacer(), text("Your profile").font_size(16.).font_weight(600)],
            vstack![
                text("Make it yours.").font_size(32.).font_weight(700),
                text("A simple, editable Rust form. Changes are saved only for this session.").font_size(16.).color(muted),
                text("DISPLAY NAME").font_size(11.).font_weight(700).color(muted),
                text_field(&app.draft_name).key("name").accessible_label("Display name").height(52.).font_size(17.).background(card).rounded(12.).on_change(|a: &mut App, v| a.draft_name = v),
                text("EMAIL ADDRESS").font_size(11.).font_weight(700).color(muted),
                text_field(&app.draft_email).key("email").accessible_label("Email address").height(52.).font_size(17.).background(card).rounded(12.).on_change(|a: &mut App, v| a.draft_email = v),
                text(&app.message).key("validation").font_size(14.).color(if app.dark { "#FFC0A9" } else { "#9C381C" }),
                button("Save profile").key("save").height(52.).font_size(17.).font_weight(600).background("#216B56").color("#FFFFFF").rounded(14.).on_click(App::save),
                text("No account. No network. Just your device.").font_size(12.).color(muted),
            ].key("form-scroll").flex(1.).scroll(Scroll::Vertical).gap(16., Axis::Vertical),
        ].gap(24., Axis::Vertical).into(),
    }
}
pub fn view(app: &App) -> Element<App> {
    vstack![content(app)]
        .padding(20., Edge::All)
        .font_family(&app.font)
        .color(if app.dark { "#F2F6F3" } else { "#182E27" })
        .background(if app.dark { "#14241F" } else { "#F4F5EF" })
        .into()
}
pub fn create_view(font: String) -> Result<MobileView<App>, String> {
    MobileView::new(
        App {
            font,
            ..App::default()
        },
        view,
    )
}
pub fn application(
    host: volt_ui_kit_mobile::HostContext,
) -> Result<volt_ui_kit_mobile::Application<App>, String> {
    let state = App {
        font: host.font_family.clone(),
        ..App::default()
    };
    Ok(volt_ui_kit_mobile::Application::new(host, state, view)?
        .allow_software_fallback(true)
        .on_appearance(|app, mode| app.dark = mode == volt_ui_kit::ThemeMode::Dark)
        .theme_with(|app, _| {
            let mut theme = if app.dark {
                volt_ui_kit::UiTheme::dark()
            } else {
                volt_ui_kit::UiTheme::light()
            };
            theme.palette.base[0] = if app.dark {
                [20. / 255., 36. / 255., 31. / 255., 1.]
            } else {
                [244. / 255., 245. / 255., 239. / 255., 1.]
            };
            theme.palette.base[1] = if app.dark {
                [32. / 255., 49. / 255., 45. / 255., 1.]
            } else {
                [1.; 4]
            };
            theme.palette.content = if app.dark {
                [242. / 255., 246. / 255., 243. / 255., 1.]
            } else {
                [24. / 255., 46. / 255., 39. / 255., 1.]
            };
            theme.palette.primary = [33. / 255., 107. / 255., 86. / 255., 1.];
            theme
        }))
}
volt_ui_kit_mobile::export_ios_app!(application);

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn actual_touch_tap_activates_once_even_when_host_draws_between_events() {
        let mut m = create_view("sans-serif".into()).unwrap();
        m.resize(402., 790., 3.).unwrap();
        let b = m
            .runtime()
            .boxes()
            .iter()
            .find(|b| b.id == "profile")
            .unwrap()
            .bounds
            .bounds();
        let p = [(b[0] + b[2] / 2.) / 3., (b[1] + b[3] / 2.) / 3.];
        m.touch_begin(p).unwrap();
        m.resize(402., 790., 3.).unwrap();
        m.touch_end(p).unwrap();
        assert_eq!(m.context().screen, Screen::Profile);
    }
    #[test]
    fn navigation_form_validation_save_and_cancel() {
        let mut m = create_view("sans-serif".into()).unwrap();
        assert!(m.activate("note-0").unwrap());
        assert_eq!(m.context().screen, Screen::Detail(0));
        assert!(m.activate("profile").unwrap());
        m.set_text("name", "").unwrap();
        m.activate("save").unwrap();
        assert_eq!(m.context().screen, Screen::Profile);
        assert!(m.context().message.contains("1–60"));
        m.set_text("name", "Zoë 東京").unwrap();
        m.set_text("email", "zoe@example.com").unwrap();
        m.activate("save").unwrap();
        assert_eq!(m.context().name, "Zoë 東京");
        assert_eq!(m.context().screen, Screen::Detail(0));
        m.activate("profile").unwrap();
        m.set_text("name", "Discard me").unwrap();
        m.activate("cancel").unwrap();
        assert_eq!(m.context().name, "Zoë 東京");
        m.activate("back").unwrap();
        assert_eq!(m.context().screen, Screen::Notes);
    }
    #[test]
    fn keyboard_resize_reveals_email_and_invalid_native_events_do_not_edit() {
        let mut m = create_view("sans-serif".into()).unwrap();
        m.activate("profile").unwrap();
        assert!(!m.focus_text("save").unwrap());
        assert!(m.focus_text("email").unwrap());
        m.resize(390., 300., 3.).unwrap();
        let email = m
            .runtime()
            .boxes()
            .iter()
            .find(|b| b.id == "email")
            .unwrap()
            .bounds
            .bounds();
        assert!(email[1] >= 0. && email[1] + email[3] <= 900.);
        let before = m.context().draft_email.clone();
        assert!(!m.set_text("not-a-field", "ignored").unwrap());
        assert_eq!(m.context().draft_email, before);
        m.touch_begin([30., 200.]).unwrap();
        m.set_suspended(true);
        m.touch_end([30., 200.]).unwrap();
        assert_eq!(m.context().screen, Screen::Profile);
        m.set_suspended(false);
        assert!(m.touch_begin([f32::NAN, 20.]).is_err());
    }
    #[test]
    fn scrolling_drag_never_activates_row_and_scale_resize_preserves_state() {
        let mut m = create_view("sans-serif".into()).unwrap();
        m.resize(390., 760., 3.).unwrap();
        let b = m
            .runtime()
            .boxes()
            .iter()
            .find(|b| b.id == "note-0")
            .unwrap()
            .bounds
            .bounds();
        let p = [(b[0] + 20.) / 3., (b[1] + 20.) / 3.];
        m.touch_begin(p).unwrap();
        m.touch_move([p[0], p[1] - 60.]).unwrap();
        m.touch_end([p[0], p[1] - 120.]).unwrap();
        assert_eq!(m.context().screen, Screen::Notes);
        assert!(m.runtime().scroll_offset("notes-list")[1] > 100.);
        m.scroll("notes-list", [0., 16384.]).unwrap();
        assert!(m.activate("note-23").unwrap());
        m.resize(760., 390., 2.).unwrap();
        assert_eq!(m.context().screen, Screen::Detail(23));
        m.set_suspended(true);
        assert!(!m.activate("profile").unwrap());
        m.set_suspended(false);
        assert!(m.activate("profile").unwrap());
        m.resize(390., 300., 3.).unwrap();
        assert!(m.set_text("name", "Still here").unwrap());
        m.resize(390., 760., 3.).unwrap();
        assert_eq!(m.context().draft_name, "Still here");
        assert!(m.resize(f32::NAN, 500., 2.).is_err());
        assert!(m.resize(390., 500., 0.1).is_err());
        assert!(m.resize(0.1, 500., 1.).is_err());
        assert_eq!(m.context().draft_name, "Still here");
        assert!(m.resize(10000., 10000., 3.).is_err());
    }
}
