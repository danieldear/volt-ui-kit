# Experimental iOS apps

**Experimental, not production iOS support.** Write your screens, state and
callbacks in Rust with Volt UI Kit. iOS supplies what people notice first
when it is wrong: the navigation bar and back swipe, scrolling, switches,
sliders, text editing and the keyboard. Volt draws each page's content, in
iOS's colours and system font by default or in your app's own look. No
per-app Swift, SwiftUI or web view is needed.

The [Android prototype](android-prototype.md) uses the same Rust application
layer and project template; there, Volt draws the navigation bar as well.

## How an iOS app is put together

```text
Your Rust app: state + view + typed callbacks
                        |
volt-ui-kit-mobile: Application, platform events, page snapshot
                        |
volt-ui-kit: layout, text, drawing (Vello on Metal)
                        |
VoltUIHost.swift, the kit's thin UIKit shell:
  UINavigationController   bar, large title, back button and swipe
  UIScrollView             momentum, bounce, indicators, tap to top
  UISwitch, UISlider       drawn by iOS where Volt lays them out
  UITextField, UITextView  typing, selection, autocorrect, dictation
  CAMetalLayer             Volt's pages, presented in step with UIKit
```

| Part | Who draws it | Notes |
|---|---|---|
| Navigation bar, large title, back | iOS | From `navigation_stack` page modifiers |
| Back swipe, from the edge or anywhere (iOS 26) | iOS | Paused while a finger drags a slider or carousel |
| Scrolling physics and indicators | iOS | Volt draws the content at UIKit's position |
| Switches and sliders | iOS | Volt lays them out and keeps your state |
| Text fields and text areas while editing | iOS | Volt draws them at rest |
| Everything else on the page | Volt | Lists, buttons, cards, grids, charts, canvases |

The Swift shell is kit code. Generated Xcode projects reference it; your app
does not contain or edit Swift.

## Create a project

Requires macOS, full Xcode, Python 3 and Rust 1.89 or newer. From a current
toolkit checkout:

```sh
cargo install --locked --path . --features dev-tools --bin cargo-volt
rustup target add aarch64-apple-ios-sim aarch64-apple-ios
cargo volt new my-ios-app --platform ios --kit-path /path/to/volt-ui-kit
cd my-ios-app
open ios/VoltApp.xcodeproj
```

Select **VoltApp** and an iPhone Simulator, then Run. Xcode builds and links
the Rust static library. Intel Macs need `x86_64-apple-ios` instead of
`aarch64-apple-ios-sim`. The deployment target is iOS 17.

The starter has a large-title home page with a counter and a list, and a
Settings page (opened from the bar) with a text field, a switch and Save with
validation. Edit **`src/lib.rs`**.

```text
my-ios-app/
  Cargo.toml               staticlib/rlib + the reusable host dependency
  src/lib.rs               your state, pages, callbacks and factory
  ios/
    VoltApp.xcodeproj/     ready to open; references the kit's Swift shell
    App/Info.plist         bundle, scene and 120 Hz configuration
    build-rust.sh          builds the Rust library during the Xcode build
    Tests/SmokeTests.swift a UI test of the starter
```

The first Xcode build resolves `Cargo.lock`; commit it. Later builds use
`--locked`. Keep the toolkit checkout where the project expects it; if it
moves, regenerate the project. Existing destinations are never overwritten.

## Run on your iPhone

Generated projects have code signing off, so the Simulator works without an
Apple account. For your phone:

1. On the iPhone, turn on **Settings → Privacy & Security → Developer Mode**
   and restart when asked. Connect it by cable and choose **Trust**.
2. In Xcode, select the **VoltApp** target → **Signing & Capabilities**, turn
   on **Automatically manage signing**, pick your team and set a bundle ID of
   your own, such as `com.yourdomain.myapp`.
3. Choose your iPhone as the destination and Run. Keep the phone unlocked
   while Xcode prepares it the first time.

From the command line instead:

```sh
xcodebuild -project ios/VoltApp.xcodeproj -scheme VoltApp \
  -destination 'id=YOUR-DEVICE-ID' -allowProvisioningUpdates \
  CODE_SIGNING_ALLOWED=YES CODE_SIGN_STYLE=Automatic \
  DEVELOPMENT_TEAM=YOUR-TEAM-ID PRODUCT_BUNDLE_IDENTIFIER=com.yourdomain.myapp build
xcrun devicectl device install app --device YOUR-DEVICE-ID \
  build/Build/Products/Debug-iphoneos/VoltApp.app
```

`xcrun devicectl list devices` shows your device ID. Signing, provisioning and
App Store distribution are yours; Volt does not store certificates or IDs.

## Pages and navigation

```rust,ignore
use volt_ui_kit_mobile::{Application, HostContext};
use volt_ui_kit_mobile::ui_kit::ui::{prelude::*, Container};

#[derive(Clone, Copy, PartialEq)]
enum Page { Home, Settings }
pub struct App { history: NavigationHistory<Page>, count: usize }

fn home(app: &App) -> Container<App> {
    vstack![button(format!("Count: {}", app.count)).key("add")
        .on_click(|a: &mut App| a.count += 1)]
    .key("home").flex(1.).scroll(Scroll::Vertical)
}
fn view(app: &App) -> Element<App> {
    navigation_stack(&app.history, |page| match page {
        Page::Home => home(app)
            .navigation_title("My app")
            .large_title(true)
            .bar_button("Settings", |a: &mut App| { a.history.push(Page::Settings); })
            .into(),
        Page::Settings => vstack![text("Settings")].navigation_title("Settings").into(),
    })
    .key("app")
    .on_back(|a: &mut App| { a.history.back(); })
    .into()
}
pub fn application(host: HostContext) -> Result<Application<App>, String> {
    Application::new(host, App { history: NavigationHistory::new(Page::Home), count: 0 }, view)
}
volt_ui_kit_mobile::export_ios_app!(application);
```

- `navigation_title`, `large_title` and `bar_button` describe a page's bar.
  `on_back` runs for the back button and the back swipe. Your
  `NavigationHistory` decides the pages; iOS animates the push and pop.
- Give each page's scroll view a key. It then runs under the bars with iOS's
  insets, its large title shrinks as it scrolls, and going back returns to the
  same scroll position.
- On desktop, the web and Android, the same code draws the bar with Volt.

## Tabs

`tab_view` gives an app its top-level sections. On iOS the host shows the
system tab bar (Liquid Glass on iOS 26) with SF Symbols and badges, and each
tab keeps its own navigation stack; elsewhere Volt draws Material's flexible
navigation bar.

```rust,ignore
tab_view(app.section, [
    tab_destination(Section::Inbox, "Inbox", Icon::Inbox).symbol("tray.fill").badge("3"),
    tab_destination(Section::Done, "Done", Icon::Check).symbol("checkmark.circle.fill"),
], navigation_stack(&app.history, |page| pages(app, page)))
.key("sections")
.on_select(open_section)   // swap in that section's history
.on_reselect(|a: &mut App, s| a.history.reset(s.root()))
```

- Labels are required, and three to five tabs read best. A tab with nothing
  in it stays; its page says why.
- Choosing the open tab again calls `on_reselect`: go back to the section's
  first page, as both platforms do.
- The gallery has three sections: Components, Shop (with the cart count as a
  badge) and Work.

## Sheets

`bottom_sheet` rests at the heights you give it (`SheetDetent::Medium`,
`Large`, or `Points`), with a grabber when there's more than one. Drag the
grabber or header to change height or swipe down to close; tap the grabber
to step through the heights. `cancel` and `done` put Cancel at the leading
edge and Done at the trailing edge; `on_dismiss_attempt` lets an app ask
before a swipe throws work away. While a sheet or dialog is open, the iOS
host fades its tab bar and navigation bar, and the page behind
takes no touches. With the keyboard up a sheet goes to full height and keeps
its fields above the keyboard. The gallery's **Sheets** page shows all of it.

## Action dock

`action_dock` puts a page's actions at the bottom of the screen, where a
thumb reaches, instead of in the top bar: one main action, up to two round
ones, and More for the rest in a sheet. It floats just above the tab bar,
home indicator or keyboard (the iOS host reports them with a `bars` event),
moves away while the list it `follows` scrolls down, and comes back as soon
as it scrolls up or reaches either end. That list gets room at its end so
its last row clears the dock.

```rust,ignore
action_dock(notes_list.key("notes"))
    .key("notes-dock")
    .follows("notes")
    .primary(dock_action("New note", Icon::Plus).on_press(|a: &mut App| a.add()))
    .secondary(dock_action("Search", Icon::Search).on_press(|a: &mut App| a.search()))
    .more([dock_action("Delete all", Icon::Trash).danger().on_press(|a: &mut App| a.clear())])
    .more_open(app.more)
    .on_more(|a: &mut App, open| a.more = open)
```

- Labels say what happens; the round actions show only their icon, so their
  label is what VoiceOver reads.
- Keep the top bar for the title and Back. `back(label, f)` adds a Back at
  the dock's leading end for deep pages.
- `search(dock_search(query, open))` adds a Search button that turns the
  dock into a search field with the keyboard up; the dock rides on the
  keyboard, and Close puts the actions back.
- The gallery's **Action dock** page shows it.

## Lists, swipe actions and Select mode

`list_row` is a phone list row; put rows in a `list_section`. Swipe actions
are shortcuts, never the only way to an action: offer the same actions in
Select mode and the dock, and follow a delete with Undo.

```rust,ignore
list_row(&note.title)
    .key(format!("note-{id}"))
    .subtitle(&note.preview)
    .value(&note.age)
    .chevron(true)
    .on_press(move |a: &mut App| a.open(id))
    .trailing_actions([swipe_action("Delete", Icon::Trash)
        .tone(SwipeTone::Danger)
        .full_swipe()
        .on_press(move |a: &mut App| a.delete_with_undo(id))])
    .swipe_open(app.open_row(id))
    .on_swipe_open(move |a: &mut App, side| a.set_open_row(id, side))
    .selecting(app.selecting)
    .selected(app.selected.contains(&id))
    .on_toggle(move |a: &mut App| a.toggle(id))
```

Touch and hold a row for its menu. Name the swipe actions there too, with
anything else that applies; `SwipeTone::Danger` shows in red:

```rust,ignore
    .hold_actions([
        swipe_action("Pin", Icon::Star).on_press(move |a: &mut App| a.pin(id)),
        swipe_action("Delete", Icon::Trash)
            .tone(SwipeTone::Danger)
            .on_press(move |a: &mut App| a.delete_with_undo(id)),
    ])
    .menu_open(app.menu == Some(id))
    .on_menu_open(move |a: &mut App, open| a.menu = open.then_some(id))
```

- `swipe_hint(true)` on the first row, the first time someone sees the
  list, nudges it aside once to show it swipes. Turn it off after.
- VoiceOver lists each swipe action and Show menu in the row's actions
  rotor; nothing on a row needs a gesture.
- Keep one row open: opening another, tapping, or acting closes it.
- In Select mode the header says how many are selected and the dock shows
  what applies to them; Done ends it.
- The gallery's **Swipe and select** page shows it all.

## Chat: composer and live text

`composer` is a chat's message box: put it last on the page, under the
transcript. The field grows as you type (up to `max_lines`), Send lights up
only when there's something to send, and Stop takes its place while a reply
comes in. It rides the keyboard, and there's no Done bar: Send leaves the
keyboard up for the next message, and dragging the transcript puts it away.

```rust,ignore
vstack![
    vstack(messages)
        .flex(1.)
        .scroll(Scroll::Vertical)
        .anchor_end(true),
    composer(&app.draft)
        .key("chat-composer")
        .on_change(|a: &mut App, text| a.draft = text)
        .on_send(|a: &mut App| a.send())
        .busy(app.replying)
        .on_stop(|a: &mut App| a.stop()),
]
```

- `live_text(&reply)` shows a reply that grows: give it the whole text so
  far each time, and what's new eases in after what was there.
- `anchor_end(true)` keeps the transcript at the newest message unless you
  scroll up to read.
- For other fields, `hide_done_button()` drops the Done bar the same way;
  the dock's search uses it too.
- The gallery's **Chat** page streams a reply from another thread.

## Flows, quick picks and freshness

- `sheet_flow` runs a task in steps inside one sheet. The first step has
  Cancel, later ones Back; the last step's button says what it does
  ("Create"). Each `flow_step` sets the sheet's height and whether it's
  ready to go on.
- `quick_picks` puts the likely answers first ("Today", "Tomorrow") with the
  full picker behind a last chip.
- `freshness_line` says how fresh the page is under its title; it turns
  amber when stale and says "Offline" plainly. A tap refreshes.
- The gallery's **Flows** page shows all three.

## Work cards and the activity pill

`work_card` is one job. Its `WorkState` sets colour, icon and words
together: Working (with progress if known), Waiting ("Needs you"), Failed,
Done. Give it the one action that applies now: Allow, Try again, Open.
While work runs in the background, `activity_pill` floats above the bottom
bars ("Importing 3 of 5"); show it only while something runs. The gallery's
Work tab shows all four states.

## Undo bar

After something is deleted, do it and offer Undo in a `snackbar` rather
than asking first. Give it to the page's dock with `notice`, or layer it
last in a `zstack!`. Closing it after a few seconds is the app's: an
`Updater` from a timer thread (see the gallery's Swipe and select page).

```rust,ignore
action_dock(list)
    .notice(
        snackbar(format!("Deleted {title}."))
            .key("undo")
            .action("Undo", |a: &mut App| a.undo())
            .open(app.can_undo())
            .on_close(|a: &mut App| a.forget_undo()),
    )
```

## Fields and busy buttons

Say what a field holds with `input_kind` and the phone does the rest: the
email keyboard, a texted one-time code above the keyboard, the password
manager for passwords, no suggestions for secrets such as API keys.
`button(..).busy(true)` shows a spinner in place of the label while the
work it started runs.

## Search

Use `search_field` for every search. On iOS it is UIKit's own search field;
on Android Volt draws Material 3's search bar.

```rust,ignore
search_field(&app.query)
    .key("notes-search")
    .placeholder("Search notes")
    .on_change(|a: &mut App, q| a.query = q)
```

- Filter as people type; `on_submit` runs on the keyboard's Search key.
- Name what's searched in the placeholder ("Search notes").
- The action dock's search and the gallery's Pantry use it.

## Alerts and action sheets

Ask only when an action can't be undone or costs something; otherwise do it
and offer Undo. `alert_dialog` asks one question with at most two answers
and Cancel; `action_sheet` offers choices. On iOS they are UIKit's own; on
Android Volt draws Material 3's.

```rust,ignore
alert_dialog("Sign out of Volt?")
    .key("sign-out")
    .message("Drafts that haven't synced will be lost.")
    .action(prompt_action("Sign out").destructive().on_press(|a: &mut App| a.sign_out()))
    .cancel("Cancel")
    .open(app.asking)
    .on_close(|a: &mut App| a.asking = false)
```

- Name answers by what they do ("Sign out"), never "OK".
- An action sheet always has Cancel; tapping outside it cancels too.
- The gallery's **Alerts** page shows both.

## Work off the UI thread

Network requests, file sync and timers run on threads of your own. They send
their results back with an `Updater`: each change runs on the UI thread, in
the order sent, and the page is drawn again. Nothing touches your state from
another thread.

```rust,ignore
use volt_ui_kit_mobile::{Application, HostContext, Updater};

pub struct App { updates: Updater<App>, reply: Option<String> }

fn ask(a: &mut App) {
    let updates = a.updates.clone();
    std::thread::spawn(move || {
        let reply = fetch_reply();
        updates.update(move |a: &mut App| a.reply = Some(reply));
    });
}
pub fn application(host: HostContext) -> Result<Application<App>, String> {
    Application::new_with_updater(host, |updates| App { updates, reply: None }, view)
}
```

- `Application::new_with_updater` hands the state its `Updater` to keep;
  `Application::updater()` returns another at any time. Clone it freely.
- Changes sent close together share one wake and one draw.
- `update` returns false, and drops the change, once the app has closed: a
  long-running worker can stop then.
- On iOS the host is woken on the main queue. The Android prototype applies
  waiting changes with its next event.
- The gallery's **Background work** page shows it: results arrive from a
  worker thread one by one.

## Colours, type and motion

- **Colours.** Without a theme, pages use iOS's own colours for the current
  appearance and contrast setting: grouped backgrounds, label colours,
  hairline separators and the app tint. `Application::theme_with` replaces them
  with your theme. `Application::theme_from_system` starts from iOS's colours
  and adjusts them, for example a brand accent on some pages only.
- **Type.** The iPhone's system font is read from the device at runtime and
  never bundled. Weights such as `font_weight(700)` come from its variable
  `wght` axis. An explicit `font_family` on your root overrides it.
- **Motion.** Press feedback and transitions run from a display link that
  stops when nothing moves. The Reduce Motion setting is followed. The
  generated `Info.plist` allows 120 Hz on ProMotion iPhones.

## The iPhone gallery

[`examples/ios-gallery`](../examples/ios-gallery) shows every part on a phone:
navigation, buttons, lists, switches and sliders, text input, typography,
badges and cards, and **Pantry**, a branded grocery shop (product grid,
carousel, product page, cart and checkout). Each page ends with what to check
on a device.

```sh
cargo test --locked --manifest-path examples/ios-gallery/rust/Cargo.toml \
  --target-dir target/ios-gallery
open examples/ios-gallery/VoltGallery.xcodeproj
```

For your phone, override the bundle prefix and team on the command line; the
UI test runner gets `VOLT_BUNDLE_ID` plus `.tests`:

```sh
cd examples/ios-gallery
xcodebuild test -project VoltGallery.xcodeproj -scheme VoltGallery \
  -destination 'id=YOUR-DEVICE-ID' -allowProvisioningUpdates \
  CODE_SIGNING_ALLOWED=YES CODE_SIGN_STYLE=Automatic \
  DEVELOPMENT_TEAM=YOUR-TEAM-ID VOLT_BUNDLE_ID=com.yourdomain.gallery
```

The UI tests drive the phone: every page through the real bar, the bar
button, momentum and the collapsing large title, the keyboard and text area,
returning to a page without moving it, and a shop order by touch. Leave the
phone alone while they run.

- `VOLT_GALLERY_PAGE=shop` (or `controls`, `text`, `lists`, ...) opens a page
  at launch, for screenshots.
- `VOLT_ANIMATION_SPEED=0.1` plays UIKit animations ten times slower, to look
  at transitions frame by frame. It works in any Volt iOS app.

Regenerate the checked-in Xcode project with XcodeGen after editing
`examples/ios-gallery/project.yml`.

The older [Field Notes](../examples/ios) example (`examples/ios`) still runs
on the same shell and keeps its own UI tests.

## Test a generated project

```sh
cargo test --locked
xcodebuild -project ios/VoltApp.xcodeproj -scheme VoltApp \
  -destination 'platform=iOS Simulator,name=iPhone 17 Pro' test
```

Use an installed Simulator name. The starter's UI test checks the counter, the
Settings bar button, native text editing, the switch, Save and scrolling to
the last row.

## Drawing and presentation

Compatible GPUs draw with Vello. The Simulator lacks some features Vello
needs, so the starter calls `.allow_software_fallback(true)`: software drawing
with Metal presentation there. The `render-proof` accessibility element
reports the backend, drawing mode and presented-frame count. Frames are
presented with Core Animation transactions, so UIKit parts on top of Volt's
content move in the same frame. Rendering stays event-driven: no frames are
drawn while nothing changes. See Apple's
[Simulator Metal restrictions](https://developer.apple.com/documentation/metal/developing-metal-apps-that-run-in-simulator).

## Host boundary

The kit keeps `forbid(unsafe_code)`; the host crate holds the narrow iOS
surface boundary. The Swift shell and Rust exchange small, bounded JSON
messages (touches, scroll positions, bar actions, colours) and a page
snapshot. This is plumbing between the shell and Rust, not a way to describe
UI. The wire decoder rejects unknown operations and fields, non-finite
geometry and out-of-range values. All calls stay on UIKit's main thread. It
is an internal ABI, not a stable third-party one.

## Remaining limits

Tested on a physical iPhone 17 Pro with the gallery's UI tests. Still missing
or partial:

- **Fallback fonts.** Emoji and scripts the system font lacks (Chinese,
  Japanese, Korean, Arabic and others) do not render yet.
- **Network images**, **bottom sheets with detents**, a **tab bar**, native
  **segmented controls**, **menus and alerts**, **date pickers** and **maps**
  on iOS.
- **Password fields** stay read-only in the native editor; secret values never
  enter snapshots.
- **Dynamic Type** and full **VoiceOver** acceptance; persistence and state
  restoration; multi-window; push notifications and other services.
- Energy and frame-time profiling, App Store packaging and a distributable
  SDK.
