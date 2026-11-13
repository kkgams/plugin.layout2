//! Validated flat document import and canonical preorder export.
use crate::model::{self, Error, Node, State};
use crate::{LayoutDocument, LayoutNode, SplitNode, Workspace};
use std::collections::HashSet;

pub(crate) fn import(document: LayoutDocument) -> model::Result<State> {
    let bad = |message| Error::new("invalid-state", message);
    if document.version != 2
        || document.revision > model::MAX_SAFE_INTEGER
        || document.next_split_id == 0
        || document.next_split_id > model::MAX_SAFE_INTEGER
        || !model::config_valid(
            document.width,
            document.height,
            document.gap,
            document.min_width,
            document.min_height,
        )
        || document.workspaces.is_empty()
        || document.workspaces.len() > 128
        || document
            .workspaces
            .iter()
            .map(|w| w.nodes.len())
            .sum::<usize>()
            > 2047
    {
        return Err(bad(
            "Unsupported version, dimensions, counters or document limits",
        ));
    }
    let mut workspace_ids = HashSet::new();
    let mut panel_ids = HashSet::new();
    let mut split_ids = HashSet::new();
    // Check every scalar and graph iteratively before constructing recursive Nodes.
    for workspace in &document.workspaces {
        if !model::valid_id(&workspace.id)
            || !model::valid_name(&workspace.name)
            || !workspace_ids.insert(workspace.id.as_str())
        {
            return Err(bad("Invalid or duplicate workspace"));
        }
        let mut local_panels = HashSet::new();
        for node in &workspace.nodes {
            match node {
                LayoutNode::Panel(id) => {
                    if !model::valid_id(id) || !panel_ids.insert(id.as_str()) {
                        return Err(bad("Invalid or duplicate panel ID"));
                    }
                    local_panels.insert(id.as_str());
                }
                LayoutNode::Split(split) => {
                    if split.id == 0
                        || split.id >= document.next_split_id
                        || !split_ids.insert(split.id)
                        || !model::valid_ratio(split.ratio)
                        || split.first as usize >= workspace.nodes.len()
                        || split.second as usize >= workspace.nodes.len()
                    {
                        return Err(bad("Invalid split ID, ratio or child index"));
                    }
                }
            }
        }
        match (workspace.root, workspace.focused.as_deref()) {
            (None, None) if workspace.nodes.is_empty() => {}
            (Some(root), Some(focused))
                if (root as usize) < workspace.nodes.len() && local_panels.contains(focused) => {}
            _ => return Err(bad("Invalid workspace root or focus")),
        }
        let mut visited = vec![false; workspace.nodes.len()];
        let mut pending = Vec::new();
        if let Some(root) = workspace.root {
            pending.push((root, 1));
        }
        while let Some((index, depth)) = pending.pop() {
            if depth > model::MAX_DEPTH || visited[index as usize] {
                return Err(bad("Tree depth, cycle or shared child violation"));
            }
            visited[index as usize] = true;
            if let LayoutNode::Split(split) = &workspace.nodes[index as usize] {
                pending.push((split.second, depth + 1));
                pending.push((split.first, depth + 1));
            }
        }
        if visited.iter().any(|seen| !seen) {
            return Err(bad("Unreachable workspace nodes"));
        }
    }
    if !workspace_ids.contains(document.active_workspace.as_str())
        || panel_ids.len() > model::MAX_PANELS
        || split_ids.len() >= model::MAX_PANELS
    {
        return Err(bad("Invalid active workspace or excessive panels/splits"));
    }
    let state = State {
        version: 1,
        revision: document.revision,
        width: document.width,
        height: document.height,
        gap: document.gap,
        min_width: document.min_width,
        min_height: document.min_height,
        next_split_id: document.next_split_id,
        active_workspace: document.active_workspace,
        workspaces: document
            .workspaces
            .into_iter()
            .map(|w| {
                let tree = w.root.map(|root| inflate(&w.nodes, root));
                model::Workspace {
                    id: w.id,
                    name: w.name,
                    focused: w.focused,
                    tree,
                }
            })
            .collect(),
    };
    state.validate()?;
    Ok(state)
}

fn inflate(nodes: &[LayoutNode], index: u32) -> Node {
    match &nodes[index as usize] {
        LayoutNode::Panel(id) => Node::leaf(id.clone()),
        LayoutNode::Split(split) => Node::Split {
            id: split.id,
            vertical: split.vertical,
            ratio: split.ratio,
            first: Box::new(inflate(nodes, split.first)),
            second: Box::new(inflate(nodes, split.second)),
        },
    }
}

fn flatten(node: Node, nodes: &mut Vec<LayoutNode>) -> u32 {
    let index = nodes.len() as u32;
    match node {
        Node::Leaf { id } => nodes.push(LayoutNode::Panel(id)),
        Node::Split {
            id,
            vertical,
            ratio,
            first,
            second,
        } => {
            nodes.push(LayoutNode::Split(SplitNode {
                id,
                vertical,
                ratio,
                first: 0,
                second: 0,
            }));
            let first = flatten(*first, nodes);
            let second = flatten(*second, nodes);
            nodes[index as usize] = LayoutNode::Split(SplitNode {
                id,
                vertical,
                ratio,
                first,
                second,
            });
        }
    }
    index
}

pub(crate) fn export(state: State) -> LayoutDocument {
    LayoutDocument {
        version: 2,
        revision: state.revision,
        width: state.width,
        height: state.height,
        gap: state.gap,
        min_width: state.min_width,
        min_height: state.min_height,
        next_split_id: state.next_split_id,
        active_workspace: state.active_workspace,
        workspaces: state
            .workspaces
            .into_iter()
            .map(|w| {
                let mut nodes = Vec::new();
                let root = w.tree.map(|tree| flatten(tree, &mut nodes));
                Workspace {
                    id: w.id,
                    name: w.name,
                    focused: w.focused,
                    root,
                    nodes,
                }
            })
            .collect(),
    }
}
