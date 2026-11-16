# Announcements and live projects checklist

Help Rust developers evaluate and adopt Volt UI Kit through working projects,
community discussions, ecosystem listings and open-source news coverage. This
is not a paid-advertising plan or permission to submit posts automatically.

Last reviewed: 2026-11-11. All tasks below are pending; existing examples are
starting points, not completed public adoption projects. Complete the
[release readiness checklist](public-release.md) before a broad release
announcement. Recheck each community's rules immediately before submitting.

## 1. Establish an accurate introduction

- [ ] Describe Volt as a Rust-native UI toolkit and explain its actual design
  choices, supported platforms, license and pre-1.0 API expectations.
- [ ] Explain who it suits: developers building Rust desktop tools, settings
  applications, editors, utilities and agent interfaces.
- [ ] Distinguish native desktop rendering from browser demos and experimental
  mobile hosts; do not imply feature parity across them.
- [ ] Present trade-offs relative to egui/Iced respectfully, using reproducible
  examples rather than unsupported rankings or claims of universal superiority.
- [ ] Avoid unmeasured performance claims, fake testimonials, vote requests,
  misleading platform screenshots and promises of automatic migration.

## 2. Prepare the projects developers can actually use

### Live Catalog and runnable recipes

- [ ] Publish the final matching Catalog/WASM revision with no signup barrier.
- [ ] Verify light/dark themes, navigation, forms, menus, scrolling, tables and
  copied code on the deployed site, not only a local development server.
- [ ] Keep native-only capabilities visibly identified in browser previews.
- [ ] Verify all eight runnable recipes; distinguish complete programs from
  remaining Catalog excerpts with external callbacks/helpers.

### Complete starter applications

- [ ] Finish a small settings application: navigation, editable/validated forms,
  scrolling, themes and persisted application-owned settings.
- [ ] Finish a data/file explorer: search/filter, sortable table/tree, contextual
  actions and honest empty/loading/error states, using synthetic demo data.
- [ ] Finish one editor or agent-workbench example: navigation sidebar, tabs,
  editable content, background work and unsaved-close handling where supported.
- [ ] Reuse current `rust_settings`, Catalog, Northbound, Relay or showcase
  building blocks; do not create another disconnected component gallery.
- [ ] Provide exact prerequisites, launch commands, expected behavior and known
  platform limitations for every selected project.
- [ ] Verify clean-clone builds and at least the platforms used in its footage;
  document any network services, credentials, permissions or external assets.

### Mobile application starters

- [ ] Demonstrate the existing mobile generator evolving into usable minimal,
  navigation, tabs and settings presets, not just an empty rendering surface.
- [ ] Show optional headers, scrollable bodies and footers/bottom navigation,
  with safe areas, keyboard handling, platform Back and light/dark themes.
- [ ] Demonstrate explicitly selected permissions only where native request and
  result handling actually exist; include denial and unavailable states.
- [ ] Provide reproducible generate/build/run/customize instructions and tests,
  identifying platform prerequisites and signing/device requirements.
- [ ] Complete the [mobile scaffolding gates](public-release.md#mobile-application-scaffolding)
  before describing mobile support as ready; label prototype footage honestly.

### Existing applications and external adoption

- [ ] Verify which current Volt consumers can be shown publicly; obtain approval
  before linking a private repository, publishing screenshots or naming users.
- [ ] Revalidate demonstrated workflows in Disk Grove, Volt Editor, a Markdown
  application or other chosen consumers without independently patching vendors.
- [ ] Add a public projects/showcase list with repository, license, build steps,
  toolkit revision, platform and development status for each approved entry.
- [ ] Invite a small group of developers to create their own application and
  record onboarding problems; fix blockers before the broad announcement.
- [ ] Publish external examples or testimonials only with their authors' consent.

## 3. Support coding agents with skills and plugins

- [ ] Ship one portable `volt-ui` skill with shared, progressively loaded
  references rather than separate, drifting instructions for each agent.
- [ ] Teach the skill to inspect the installed toolkit version/features, use
  actual APIs and recipes, preserve application logic, and compile/test results.
- [ ] Include component composition, state/callbacks, layout, themes, scrolling,
  keyboard/focus, accessibility and platform capability boundaries.
- [ ] Publish concise agent-readable Markdown/API references and an `llms.txt`
  entry point, with working public links and explicit version/revision matching.
  Discovery files do not automatically train models or guarantee their use.
- [ ] Generate references from maintained source/examples where practical;
  compile copied snippets and check links/version consistency in CI.
- [ ] Package the shared skill/references as lightweight plugins for Claude Code
  and Codex; validate each product's current manifest and supported components.
- [ ] Document installation, invocation, update/removal and toolkit compatibility
  for both packages; test installation in clean projects and new agent sessions.
- [ ] Keep initial packages free of MCP servers, background services and automatic
  hooks unless a concrete requirement justifies them; installation must not
  silently change global agent settings or grant additional permissions.
- [ ] Add optional, explicit project-template support for agent instructions;
  do not require an AI tool to use Volt or overwrite existing project guidance.
- [ ] Run the same acceptance tasks separately in Claude Code and Codex, with
  and without the package: create a navigation application with editable forms,
  a sortable table, scrolling and themes, then build and exercise its workflows.
- [ ] Record the toolkit/plugin revisions, agent/model versions, prompts,
  compilation success, invented API usage, correction attempts and working
  interactions. Static snippet checks alone are not agent acceptance tests.
- [ ] Publish only verified capabilities. A plugin improves access to instructions
  and recipes; it does not guarantee correct output or automatic recommendation
  of Volt over other toolkits.
- [ ] Review official [Codex packaging documentation](https://developers.openai.com/plugins/build/plugins),
  [Claude Code plugin documentation](https://code.claude.com/docs/en/plugins)
  and the [Agent Skills specification](https://agentskills.io/specification)
  before distribution; treat public directory submission as a separate approval.

## 4. Help egui and Iced developers evaluate Volt

- [ ] Write a "Coming from egui" guide and a "Coming from Iced" guide.
- [ ] Recreate equivalent small applications in Volt: counter, editable form,
  navigation and sortable list/table. Check APIs against current releases.
- [ ] Explain state ownership, callbacks, keys, composition, layout, theme,
  keyboard/focus behavior and background work in concrete examples.
- [ ] Show how application-owned Rust business logic can remain separate from
  the UI; state where reuse needs adaptation, not a mechanical translation.
- [ ] Include unsupported features, migration costs and known limitations.
- [ ] Have a developer unfamiliar with Volt follow each guide without assistance.

## 5. Prepare the material to share

- [ ] Record a short native walkthrough with working interactions and the Rust
  declaration visible; identify OS, scale, renderer and release revision.
- [ ] Capture consistent light/dark screenshots with synthetic data and no
  personal paths, account details, notification history or other private content.
- [ ] Record actual Ubuntu/KDE or other claimed Linux behavior before approaching
  Linux publications; macOS footage is not Linux runtime evidence.
- [ ] Prepare a factual brief: purpose, distinctive choices, limitations, license,
  repository, live Catalog, release notes and documented native launch command.
- [ ] Prepare install/troubleshooting answers and a clear feedback/issue route.
- [ ] Keep materials accessible without paywalls, email collection or broken links.

## 6. Community and editorial submission checklist

Track each submission's date, URL, applicable rules and outcome below. A sent
submission is not accepted coverage. Disclose that we maintain the project and
tailor the contribution to the community rather than repeating the same post.

| Done | Destination | Appropriate contribution / condition | Submission date and URL / outcome |
| --- | --- | --- | --- |
| [ ] | [Rust Users announcements](https://users.rust-lang.org/c/announcements/6) | Release, working example, trade-offs, limitations and a specific feedback request | Pending |
| [ ] | [Are We GUI Yet?](https://areweguiyet.com/) | Ecosystem listing or substantive GUI development/news entry following contribution instructions | Pending |
| [ ] | [Hacker News](https://news.ycombinator.com/showhn.html) | Show HN linking to a usable project/demo; maintainer available to answer technical questions | Pending |
| [ ] | [r/rust](https://www.reddit.com/r/rust/) | Weekly "What's Everyone Working on This Week?" thread initially; qualifying technical article later | Pending |
| [ ] | [This Week in Rust](https://github.com/rust-lang/this-week-in-rust) | Eligible human-written technical article or a suitable contribution opportunity; not a project-update PR | Pending |
| [ ] | Rust GUI Discord/forum channels | Designated showcase channel; follow local rules and answer implementation questions | Pending |
| [ ] | [OMG! Ubuntu](https://www.omgubuntu.co.uk/contact) | Developer news pitch with an actual Linux demonstration, release link and reliable build steps | Pending |
| [ ] | [OMG! Linux](https://www.omglinux.com/contact-tip-form/) | Relevant Linux/open-source news suggestion; coordinate rather than duplicate pitches to sister publications | Pending |
| [ ] | [It's FOSS](https://itsfoss.com/contact-us/) | Open-source release suggestion with the Linux story, screenshots and source | Pending |
| [ ] | [Awesome Rust](https://github.com/rust-unofficial/awesome-rust/blob/main/CONTRIBUTING.md) | Evaluate current eligibility, then submit an accurate listing if it qualifies | Pending |
| [ ] | Relevant local Rust meetups | Short live demonstration and technical discussion, agreed with the organizer | Pending |
| [ ] | Other open-source communities/publications | Research relevance and submission rules first; record the verified destination before posting | Pending |

### Rules that affect what we can submit

- [ ] Recheck [HN guidelines](https://news.ycombinator.com/newsguidelines.html)
  and [Show HN requirements](https://news.ycombinator.com/showhn.html). HN
  prohibits generated/AI-edited posts and comments, automated posting and vote
  solicitation. The maintainer writes and submits their own text; AI-prepared
  factual notes or assets are not ready-to-paste HN posts.
- [ ] Recheck the [r/rust announcement policy](https://www.reddit.com/r/rust/comments/1wkmzun/no_more_code_dumps/).
  Repository-only code dumps are not accepted; the weekly thread is the initial
  route. Library announcements have additional eligibility requirements, and
  generated articles/posts are not an acceptable workaround.
- [ ] Recheck [This Week in Rust submission rules](https://github.com/rust-lang/this-week-in-rust#llm-written-articles).
  LLM-written articles and project/tooling-update PR submissions are not accepted.
- [ ] Do not submit to a prohibited destination, invent a human endorsement,
  conceal affiliation or contact users through unsolicited mass messages.

## 7. Execute in stages and support adopters

- [ ] Stage 1: complete release gates and private developer feedback; fix failed
  installs, confusing documentation and misleading capability claims.
- [ ] Stage 2: publish the approved release and matching live examples; verify
  actual registry installation and docs.rs before changing installation claims.
- [ ] Stage 3: introduce Volt to Rust communities and ecosystem directories,
  staggering submissions so the maintainer can respond thoughtfully.
- [ ] Stage 4: approach Linux publications only after Linux runtime acceptance;
  publication is an editorial decision, not guaranteed by a submission.
- [ ] Respond to reproducible reports; publish fixes and update known issues.
- [ ] Track successful first applications, recurring onboarding failures and
  independent projects, not just stars or page views. Do not add telemetry by default.
- [ ] Review feedback after two weeks and one month; improve documentation and
  runtime behavior before adding more outreach or unrelated features.
- [ ] Keep a follow-up/patch-release plan and share substantive progress rather
  than repeatedly reposting the initial announcement.

## Completion criteria

- [ ] The release can be installed using the documented path and its public demo
  works on the advertised platforms.
- [ ] At least one complete starter and one realistic application workflow are
  public, reproducible and accurately labeled.
- [ ] egui/Iced evaluation guides are executable and honest about migration gaps.
- [ ] The shared Volt skill and Claude Code/Codex plugin packages install
  correctly and pass recorded agent-driven application acceptance tasks.
- [ ] Approved submissions are tracked with links and outcomes, with a maintainer
  available to support the developers who try Volt.

Nothing in this checklist authorizes registry publication, posting through an
account, sharing private project information or changing repository visibility.
