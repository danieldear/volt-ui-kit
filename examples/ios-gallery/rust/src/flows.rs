//! The Flows page: a freshness line, quick picks, and a task in steps
//! inside one sheet.
use super::*;

pub struct Flows {
    /// When the page last updated, and whether a refresh is running.
    pub updated: std::time::Instant,
    pub refreshing: bool,
    pub offline: bool,
    pub due: Option<usize>,
    pub creating: bool,
    pub step: usize,
    pub name: String,
    pub agent: Option<usize>,
    pub created: Vec<String>,
    pub model: usize,
    pub cellular: bool,
}
impl Default for Flows {
    fn default() -> Self {
        Self {
            // As if it last updated 20 minutes ago: stale.
            updated: std::time::Instant::now() - std::time::Duration::from_secs(20 * 60),
            refreshing: false,
            offline: false,
            due: None,
            creating: false,
            step: 0,
            name: String::new(),
            agent: None,
            created: Vec::new(),
            model: 0,
            cellular: true,
        }
    }
}

const AGENTS: [&str; 3] = ["Claude", "Codex", "Gemini"];

fn refresh(a: &mut App) {
    a.flows.refreshing = true;
    let Some(updates) = a.updates.clone() else {
        a.flows.refreshing = false;
        a.flows.updated = std::time::Instant::now();
        return;
    };
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(900));
        updates.update(|a: &mut App| {
            a.flows.refreshing = false;
            a.flows.updated = std::time::Instant::now();
        });
    });
}

pub fn page(app: &App) -> Container<App> {
    let f = &app.flows;
    let mut parts: Vec<Element<App>> = vec![
        freshness_line(Some(f.updated.elapsed()))
            .key("flows-fresh")
            .offline(f.offline)
            .refreshing(f.refreshing)
            .on_refresh(refresh)
            .into(),
        vstack![
            text("Due")
                .font_size(15.)
                .font_weight(600)
                .color(ColorToken::Muted),
            quick_picks(["Today", "Tomorrow", "Next week"])
                .key("flows-due")
                .selected(f.due)
                .on_pick(|a: &mut App, i| a.flows.due = Some(i))
                .more("Pick a date", |_: &mut App| {}),
            text("The likely answers first; the full picker is behind the last chip.")
                .font_size(13.)
                .color(ColorToken::Muted),
        ]
        .gap(8., Axis::Vertical)
        .into(),
        list_section(
            "Model",
            ["Claude Opus", "GPT-5", "Gemini Pro"]
                .into_iter()
                .enumerate()
                .map(|(i, name)| {
                    list_row(name)
                        .key(format!("flows-model-{i}"))
                        .checked(f.model == i)
                        .on_press(move |a: &mut App| a.flows.model = i)
                        .into()
                })
                .chain([list_row("Sync over mobile data")
                    .key("flows-cellular")
                    .trailing(
                        toggle("", f.cellular)
                            .key("flows-cellular-switch")
                            .accessible_label("Sync over mobile data")
                            .on_change(|a: &mut App, v| a.flows.cellular = v),
                    )
                    .into()]),
            "One choice gets a checkmark, as in Settings.",
        )
        .into(),
        primary("New project")
            .key("flows-new")
            .on_click(|a: &mut App| {
                a.flows.creating = true;
                a.flows.step = 0;
            })
            .into(),
    ];
    for (i, p) in f.created.iter().enumerate() {
        parts.push(
            work_card(p.clone())
                .key(format!("flows-project-{i}"))
                .state(WorkState::Done)
                .detail("Created")
                .into(),
        );
    }
    let name = text_field(f.name.clone())
        .key("flows-name")
        .placeholder("Project name")
        .on_change(|a: &mut App, v| a.flows.name = v)
        .height(44.)
        .font_size(17.);
    let agent = quick_picks(AGENTS)
        .key("flows-agent")
        .selected(f.agent)
        .on_pick(|a: &mut App, i| a.flows.agent = Some(i));
    let review = text(format!(
        "{} with {}",
        if f.name.trim().is_empty() {
            "Untitled"
        } else {
            f.name.trim()
        },
        f.agent.map_or("no agent", |i| AGENTS[i])
    ))
    .font_size(17.);
    let flow = sheet_flow(f.step)
        .key("flows-sheet")
        .step(
            flow_step("Name", name)
                .height(240.)
                .ready(!f.name.trim().is_empty()),
        )
        .step(
            flow_step("Agent", agent)
                .height(240.)
                .ready(f.agent.is_some()),
        )
        .step(flow_step("Review", review).height(220.))
        .finish("Create")
        .open(f.creating)
        .on_step(|a: &mut App, i| a.flows.step = i)
        .on_finish(|a: &mut App| {
            let name = a.flows.name.trim().to_owned();
            a.flows.created.push(name);
            a.flows.creating = false;
            a.flows.name.clear();
            a.flows.agent = None;
        })
        .on_cancel(|a: &mut App| a.flows.creating = false);
    zstack![
        super::page("flows-list", parts)
            .width_fraction(1.)
            .height_fraction(1.),
        flow,
    ]
    .key("flows-screen")
    .flex(1.)
}
