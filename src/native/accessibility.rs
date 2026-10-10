//! AccessKit bridge: exposes the runtime's toolkit-neutral accessibility
//! snapshot to platform screen readers (VoiceOver, Narrator/NVDA, Orca) and
//! turns their action requests back into runtime commands.
use crate::declarative::{AccessNode, AccessRole, AccessValue};
use accesskit::{
    Action, ActionData, ActionRequest, Node, NodeId, Rect, Role, Toggled, TreeId, TreeInfo,
    TreeUpdate,
};
use std::collections::HashMap;

pub(super) const ROOT: NodeId = NodeId(0);

/// Stable 64-bit id for a runtime key (FNV-1a); 0 is reserved for the window.
pub(super) fn node_id(key: &str) -> NodeId {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in key.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    NodeId(hash.max(1))
}

/// A screen-reader request, addressed by runtime key.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum AccessCommand {
    Focus(String),
    Click(String),
    SetValue(String, AccessValue),
    Step(String, bool),
}

/// Build a full tree update and the id map used to route later actions.
pub(super) fn tree_update(
    nodes: &[AccessNode],
    title: &str,
    size: [f32; 2],
) -> (TreeUpdate, HashMap<NodeId, String>) {
    let ids: Vec<NodeId> = nodes.iter().map(|n| node_id(&n.id)).collect();
    let mut children: Vec<Vec<NodeId>> = vec![Vec::new(); nodes.len()];
    let mut roots = Vec::new();
    for (i, n) in nodes.iter().enumerate() {
        match n.parent {
            Some(p) => children[p].push(ids[i]),
            None => roots.push(ids[i]),
        }
    }
    let mut window = Node::new(Role::Window);
    window.set_label(title);
    window.set_bounds(Rect::new(0., 0., f64::from(size[0]), f64::from(size[1])));
    window.set_children(roots);
    let mut out = vec![(ROOT, window)];
    let mut focus = ROOT;
    let mut map = HashMap::with_capacity(nodes.len());
    for ((n, id), kids) in nodes.iter().zip(&ids).zip(children) {
        let role = match n.role {
            AccessRole::Group => Role::Group,
            AccessRole::ScrollView => Role::ScrollView,
            AccessRole::Label => Role::Label,
            AccessRole::Button => Role::Button,
            AccessRole::CheckBox => Role::CheckBox,
            AccessRole::Switch => Role::Switch,
            AccessRole::TextInput if n.password => Role::PasswordInput,
            AccessRole::TextInput => Role::TextInput,
            AccessRole::MultilineTextInput => Role::MultilineTextInput,
            AccessRole::Slider => Role::Slider,
        };
        let mut node = Node::new(role);
        let [x, y, w, h] = n.bounds.map(f64::from);
        node.set_bounds(Rect::new(x, y, x + w, y + h));
        let input = matches!(
            n.role,
            AccessRole::TextInput | AccessRole::MultilineTextInput
        );
        if input {
            if !n.label.is_empty() {
                node.set_placeholder(n.label.as_str());
            }
        } else if !n.label.is_empty() {
            node.set_label(n.label.as_str());
        }
        if let Some(value) = &n.value {
            node.set_value(value.as_str());
        }
        if let Some([value, min, max, step]) = n.numeric {
            node.set_numeric_value(f64::from(value));
            node.set_min_numeric_value(f64::from(min));
            node.set_max_numeric_value(f64::from(max));
            node.set_numeric_value_step(f64::from(step));
        }
        if let Some(checked) = n.checked {
            node.set_toggled(if checked {
                Toggled::True
            } else {
                Toggled::False
            });
        }
        if n.read_only {
            node.set_read_only();
        }
        if !n.enabled {
            node.set_disabled();
        } else {
            match n.role {
                AccessRole::Button | AccessRole::CheckBox | AccessRole::Switch => {
                    node.add_action(Action::Focus);
                    node.add_action(Action::Click);
                }
                AccessRole::TextInput | AccessRole::MultilineTextInput => {
                    node.add_action(Action::Focus);
                    if !n.read_only {
                        node.add_action(Action::SetValue);
                    }
                }
                AccessRole::Slider => {
                    for action in [
                        Action::Focus,
                        Action::SetValue,
                        Action::Increment,
                        Action::Decrement,
                    ] {
                        node.add_action(action);
                    }
                }
                _ => {}
            }
        }
        if !kids.is_empty() {
            node.set_children(kids);
        }
        if n.focused {
            focus = *id;
        }
        map.insert(*id, n.id.clone());
        out.push((*id, node));
    }
    let mut info = TreeInfo::new(ROOT);
    info.toolkit_name = Some("Volt UI Kit".into());
    info.toolkit_version = Some(env!("CARGO_PKG_VERSION").into());
    let update = TreeUpdate {
        nodes: out,
        tree: Some(info),
        tree_id: TreeId::ROOT,
        focus,
    };
    (update, map)
}

/// Translate an action request into a runtime command, if it targets a node
/// in the current tree and is one we support.
pub(super) fn command(
    request: &ActionRequest,
    ids: &HashMap<NodeId, String>,
) -> Option<AccessCommand> {
    let key = ids.get(&request.target_node)?.clone();
    Some(match (request.action, &request.data) {
        (Action::Focus, _) => AccessCommand::Focus(key),
        (Action::Click, _) => AccessCommand::Click(key),
        (Action::Increment, _) => AccessCommand::Step(key, true),
        (Action::Decrement, _) => AccessCommand::Step(key, false),
        (Action::SetValue, Some(ActionData::Value(text))) => {
            AccessCommand::SetValue(key, AccessValue::Text(text.to_string()))
        }
        (Action::SetValue, Some(ActionData::NumericValue(n))) => {
            AccessCommand::SetValue(key, AccessValue::Number(*n as f32))
        }
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn node(id: &str, role: AccessRole, parent: Option<usize>) -> AccessNode {
        AccessNode {
            id: id.into(),
            parent,
            role,
            label: id.into(),
            value: None,
            numeric: None,
            checked: None,
            bounds: [1., 2., 30., 40.],
            enabled: true,
            focused: false,
            password: false,
            read_only: false,
        }
    }
    #[test]
    fn tree_links_children_focus_and_routes_actions() {
        let mut slider = node("volume", AccessRole::Slider, Some(0));
        slider.numeric = Some([3., 0., 10., 1.]);
        slider.focused = true;
        let nodes = vec![node("list", AccessRole::ScrollView, None), slider];
        let (update, ids) = tree_update(&nodes, "App", [800., 600.]);
        assert_eq!(update.focus, node_id("volume"));
        let root = &update.nodes[0].1;
        assert_eq!(root.children(), &[node_id("list")]);
        let list = update
            .nodes
            .iter()
            .find(|(id, _)| *id == node_id("list"))
            .unwrap();
        assert_eq!(list.1.children(), &[node_id("volume")]);
        let volume = update
            .nodes
            .iter()
            .find(|(id, _)| *id == node_id("volume"))
            .unwrap();
        assert_eq!(volume.1.numeric_value(), Some(3.));
        assert!(volume.1.supports_action(Action::Increment));
        let bounds = volume.1.bounds().unwrap();
        assert_eq!((bounds.x0, bounds.y1), (1., 42.));
        let request = ActionRequest {
            action: Action::SetValue,
            target_tree: TreeId::ROOT,
            target_node: node_id("volume"),
            data: Some(ActionData::NumericValue(6.)),
        };
        assert_eq!(
            command(&request, &ids),
            Some(AccessCommand::SetValue(
                "volume".into(),
                AccessValue::Number(6.)
            ))
        );
        let unknown = ActionRequest {
            target_node: NodeId(42),
            ..request
        };
        assert_eq!(command(&unknown, &ids), None);
    }
    #[test]
    fn disabled_controls_offer_no_actions_and_ids_are_stable() {
        let mut save = node("save", AccessRole::Button, None);
        save.enabled = false;
        let (update, _) = tree_update(&[save], "App", [10., 10.]);
        assert!(update.nodes[1].1.is_disabled());
        assert!(!update.nodes[1].1.supports_action(Action::Click));
        assert_eq!(node_id("save"), node_id("save"));
        assert_ne!(node_id("save"), node_id("cancel"));
        assert_ne!(node_id(""), ROOT);
    }
}
