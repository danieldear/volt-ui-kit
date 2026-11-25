# Volt on phones: a design review

Volt's components were built for desktop windows first. This review compares
each one with Apple's Human Interface Guidelines (iOS 26) and Material 3
Expressive (Android 16), says where the platforms themselves get things wrong,
and sets out what Volt should do instead, including components neither
platform has.

Sources, read November 2026: Apple HIG *Tab bars*, *Sheets*; Material 3
*Navigation bar* and *Bottom sheets* specs, plus the rest of each guideline set.

## What Volt believes about phones

1. **The thumb decides.** A phone is held in one hand, and the top of a 6.9"
   screen is out of reach. Primary actions live in the bottom third. Both
   platforms still put the most-used actions (Back, Edit, Add, Done) in the
   top bar; Volt doesn't make apps do that.
2. **Native where the platform is better, Volt where it's wrong.** iOS draws
   Liquid Glass, sheets, the keyboard and scrolling better than any toolkit
   could; Volt uses them. Where the platform's own pattern hurts people (see
   below), Volt offers a better default and keeps the platform version a
   flag away.
3. **Undo beats asking.** Confirmation alerts train people to tap OK
   without reading. Volt's default for destructive actions is *do it, then
   offer Undo*; a confirmation is for what can't be undone.
4. **Nothing is only a gesture.** Swipe actions, long-press menus and
   pull-to-refresh are fast for people who know them and invisible to
   everyone else. Every gesture in Volt has a visible way to do the same
   thing.
5. **State is colour plus shape plus words.** Working, waiting, failed and
   done never rely on colour alone (colour-blind people, glare, grayscale).
6. **Motion explains, never decorates.** Things move from where they came
   from and to where they went. Springs, not curves; Reduce Motion becomes
   cross-fades, not nothing.

## Where iOS and Android get it wrong

| Pattern | The problem | Volt's answer |
|---|---|---|
| Actions in the top bar (iOS and Android) | Out of thumb reach on every modern phone; Edit/Add/Done are the most-tapped buttons on a screen. | **Action dock**: a page's primary actions sit above the tab bar, in reach. The top bar keeps the title and Back. |
| Back only top-left (iOS) | The hardest spot to reach for right-handed people. Edge swipe helps but is invisible. | Native back swipe *plus* full-width swipe (iOS 26 has it) *plus* a Back in the action dock on deep pages. |
| Liquid Glass over busy content (iOS 26) | Labels on glass can drop below 4.5:1 contrast over photos and coloured content; Apple's own guidance warns about it. | Volt measures what's behind a glass bar and raises its tint when contrast would fail. "Reduce transparency" makes bars solid. |
| Tab bar minimises on scroll (iOS 26) | Labels vanish; people lose where they are. | Optional, off by default. When on, the minimised bar keeps the current tab's label. |
| Hamburger drawers (older Android) | Hidden navigation is used far less. | No drawer navigation on phones. Up to five tabs, then a "More" page *list* (not a hidden menu) for the rest. |
| FAB covering content (Android) | Covers the last row of every list, and a single action rarely deserves it. | The action dock replaces the FAB. `fab` stays for apps that want it, and lists get bottom padding for it automatically. |
| Confirmation alerts for deletes (both) | Interrupting, and people stop reading them. | **Undo bar** with a countdown ring. A confirmation only for what really can't come back. |
| Snackbars and toasts that vanish (both) | Gone before slow readers, screen readers or someone looking away finish. Material's 4–10 s timer is too short for some people. | Toasts pause while touched, while VoiceOver reads them, and while the app is in the background. Anything with an action stays until it's handled or replaced. |
| Swipe actions (both) | Invisible, easy to trigger by accident, destructive swipe with no warning. | Rows hint their swipe once on first use, a full swipe needs a short hold to fire a destructive action, and Undo always follows. The same actions are in the row's long-press menu and in Select mode. |
| Long-press menus (both) | Invisible. | A row with a menu shows a "…" affordance on selection or focus, and the menu is the same one. |
| Nested sheets (both) | People lose track of where they are; Apple's guidance says don't, apps do it anyway. | **Sheet flow**: one sheet with steps inside it (Back inside, a step line at the top) instead of sheets on sheets. Volt refuses to present a second modal sheet over one. |
| Wheel date pickers (iOS) | Slow for dates far away; hard to type a known date. | Calendar for picking, typed field for known dates, quick chips (Today, Tomorrow, Next week) for common choices. |
| Pull to refresh as the only refresh (both) | Undiscoverable; no help with stale data. | Pull to refresh *and* a "Updated 2 min ago" line that is itself a refresh button. |
| Infinite scroll (both) | No sense of position, footer unreachable. | Lists end with "Show N older …" (Volt's existing rule), with a position indicator while flinging. |
| Material You dynamic colour | An app's brand and its status colours can lose contrast or meaning. | Dynamic colour applies to surfaces and accent only; status colours (working, waiting, failed, done) keep their meaning and are checked for contrast. |
| Icon-only tab bars (some Android apps) | Icons alone are ambiguous. | Labels are required in `tab_view`. |

## Component by component

Legend: **native** = the platform draws it inside Volt's layout; **drawn** =
Volt draws it; *keep*, *fix*, *redesign*, *new*.

### Navigation

| Component | iOS | Android | Volt today | Plan |
|---|---|---|---|---|
| Navigation stack | UINavigationController, large titles, back swipe | Top app bar, predictive back | native on iOS; drawn elsewhere | *keep*. Add predictive-back preview on Android. |
| **Tab view** | Floating Liquid Glass tab bar, filled SF Symbols, search tab | Flexible navigation bar, 64 dp, pill indicator | none: `tab_bar` is a desktop tab strip | *new*: `tab_view`, native UITabBarController on iOS; Material flexible bar drawn elsewhere; per-tab history; badges. |
| Top tabs / segmented | UISegmentedControl | Tabs (primary, secondary) | `segmented_control`, `tab_bar` | *fix*: 32 pt tall, sliding thumb with spring, haptic tick; scrollable top tabs for > 4. |
| Search | Search field in bar, search tab | Search bar / search view | none on mobile | *new*: `search_bar`, bottom-placed by default (Safari's lesson), native UISearchController option. |
| Split view | Sidebar on iPad | List-detail | `navigation_split_view` | *keep*; collapses to stack on phones. |
| Pagination | — | — | `pagination` | *don't use on phones*; lists end with "Show N older". |
| Breadcrumb | — | — | `breadcrumb` | desktop only. |

### Surfaces and overlays

| Component | iOS | Android | Volt today | Plan |
|---|---|---|---|---|
| **Bottom sheet** | UISheetPresentationController, detents, grabber, swipe down | Modal/standard sheet, 28 dp top corners, 32×4 handle | drawn; all corners rounded but border straight; no handle; no detents | *redesign*: native on iOS; drawn sheet with top corners only, no border, handle, detents, spring drag, scrim, 640 dp max width. |
| Dialog / alert | UIAlertController, 2–3 buttons | Basic dialog, 28 dp corners | drawn `dialog`, `alert` | *fix*: native alerts on iOS; Material dialog drawn; stacked buttons when labels are long; destructive in red; Cancel never the default. |
| Action sheet | Action sheet / menu from button | Bottom sheet of actions | none | *new*: `action_sheet`, native on iOS, sheet of rows elsewhere. |
| Context menu | UIContextMenuInteraction with preview | Menu | drawn `context_menu` | *fix*: native on iOS with lifted preview; visible "…" affordance. |
| Popover | Popover (becomes sheet on iPhone) | Menu / tooltip | drawn | *fix*: on phones, becomes a sheet automatically. |
| Toast / snackbar | (none; banners) | Snackbar above nav bar | drawn `toaster` | *redesign*: snackbar above the tab bar and dock, pause rules above, swipe to dismiss. |
| Drawer | — | Navigation drawer (discouraged) | drawn | desktop and tablet only. |
| Tooltip / hover card | — | Tooltips on long press | hover-based | *fix*: long press shows the tooltip on touch. |

### Controls

| Component | iOS | Android | Volt today | Plan |
|---|---|---|---|---|
| Button | Filled, tinted, gray, plain, glass; 44 pt minimum | Filled, tonal, outlined, text, elevated; M3E shape morph on press | drawn | *fix*: 44 pt targets enforced, press scale and shape morph, haptic on primary actions, loading state with spinner in place. |
| Switch | UISwitch | M3 switch with icon | native on iOS | *keep*; Material switch drawn elsewhere. |
| Slider | UISlider | M3E slider with stops | native on iOS | *keep*. |
| Checkbox / radio | (lists use checkmarks) | Checkbox, radio | drawn | *fix*: on iOS, selection lists use trailing checkmarks, not boxes. |
| Stepper | UIStepper | — | `number_input` | *new*: `stepper` with press-and-hold repeat and haptics. |
| Text field | UITextField | Filled / outlined fields | native editing on iOS | *fix*: floating label, inline error, clear button, correct keyboard type and content type (email, one-time code, password). |
| Text area | UITextView | Multi-line field | native editing | *fix*: grows with content up to a limit. |
| Date and time | Inline calendar / compact / wheels | Date picker dialog, docked, modal input | drawn calendar | *redesign*: see quick chips above. |
| Select | Pop-up button menu | Exposed dropdown | drawn | *fix*: native menu on iOS; sheet list on phones for > 7 options. |
| OTP | One-time code from Messages | SMS retriever | drawn `otp_input` | *fix*: `oneTimeCode` content type so the code is offered above the keyboard. |
| Rating, tag input, combobox, colour picker | — | — | drawn | *keep*, touch-size check. |

### Content

| Component | iOS | Android | Volt today | Plan |
|---|---|---|---|---|
| List rows | Inset grouped, 44 pt, chevrons, swipe actions | List items 56/72/88 dp | drawn rows | *new*: `list_row` (leading, title, subtitle, trailing, accessory) with swipe actions and selection mode. |
| Cards | — | Elevated, filled, outlined | drawn | *keep*; consistent corner tokens. |
| Avatar, badge | — | — | drawn | *fix*: badge on tabs and icons as red oval (iOS) / small dot and large badge (Material). |
| Empty state | `ContentUnavailableView` | — | drawn `empty_state` | *fix*: always an action, never only a message. |
| Skeleton, progress, spinner | Activity indicator | Linear/circular, M3E wavy | drawn | *fix*: wavy progress on Android, native activity indicator on iOS. |
| Carousel | — | Multi-browse, hero, uncontained | drawn | *fix*: Material's carousel sizes; snap with spring. |

## New components

These solve problems neither platform solves well.

1. **Action dock.** The bottom-of-screen place for a page's actions, above
   the tab bar. One primary action (filled), up to two secondary ones, and
   a "…" for the rest. It slides with the keyboard and moves out of the way
   while scrolling down. Replaces top-bar actions and the FAB.
2. **Composer.** The message box for chat-style apps: grows as you type,
   rides on the keyboard, keeps attachments and a model or recipient chip in
   reach, Send morphs into Stop while a reply runs, and a draft survives
   leaving the page.
3. **Live text.** Text that streams in (model replies, logs) without the page
   jumping: holds scroll position unless you're at the end, shows a working
   caret, and announces to VoiceOver in sentences, not tokens.
4. **Work card.** One component for something that runs: working (animated),
   waiting for you (what it needs and a button), failed (why and what to do),
   done (result). Colour, shape, icon and words together.
5. **Activity pill.** A small pill above the tab bar while background work
   runs (sync, upload, an agent turn). Tap to see it; it never covers
   content and it goes away on its own. In-app Live Activity.
6. **Undo bar.** A snackbar for undoable actions, with a countdown ring and
   one Undo; several undos stack into "3 deleted · Undo all".
7. **Sheet flow.** A multi-step task inside one sheet: steps at the top,
   Back inside, Done only on the last step, the sheet's height following each
   step.
8. **Quick picks.** Chips for the most likely answers in front of any picker
   (dates, people, models), with the full picker behind "More".
9. **Freshness line.** "Updated 2 min ago" under a title; tap to refresh,
   turns amber when stale, shows offline plainly.
10. **Select mode.** One place for multi-select on phones: tap Select, rows
    get checkmarks, the action dock shows what applies to the selection.

## Build order

1. Tab view, bottom sheet, border fix (done; tab icons still to add).
2. Action dock, list rows with swipe actions, Select mode, long-press menu and swipe hint (done, with a short hold before a destructive full swipe).
3. Composer and live text (done).
4. Alerts and action sheets (done: native on iOS, Material 3 elsewhere), undo bar (done; toasts are the same bar without an action, and `close_after` pauses under a finger, in the background and for screen readers).
5. Work card and activity pill (done).
6. Search bar, quick picks, freshness line, sheet flow (done).
7. Field, picker and button fixes from the tables above.
8. Android host parity for everything native on iOS.

Each step ships with gallery pages, Rust tests, a simulator UI test, and
light and dark screenshots.
