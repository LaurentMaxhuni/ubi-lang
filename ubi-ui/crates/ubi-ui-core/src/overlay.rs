//! Relationships and controlled semantics shared by native overlay components.
use crate::{Error, Kind, OverlayKind, View};
use std::collections::{BTreeMap, BTreeSet};

pub fn validate_overlays(view: &View) -> Result<(), Error> {
    for node in view.nodes.values() {
        for (name, reference) in [
            ("LabelledBy", node.labelled_by.as_deref()),
            ("DescribedBy", node.described_by.as_deref()),
        ] {
            if let Some(reference) = reference {
                validate_reference(view, &node.key, reference, name)?;
            }
        }
    }

    let mut anchor_edges = BTreeMap::new();
    for node in view.nodes.values() {
        if node.kind != Kind::Overlay {
            continue;
        }
        let kind = node
            .overlay_kind
            .ok_or("OverlayKind is required on Overlay")?;
        if matches!(
            kind,
            OverlayKind::Dialog
                | OverlayKind::AlertDialog
                | OverlayKind::Drawer
                | OverlayKind::CommandPalette
        ) && node.label.trim().is_empty()
            && node.labelled_by.is_none()
        {
            return Err("modal overlays require Label or LabelledBy".into());
        }
        if let Some(anchor) = node.anchor.as_deref() {
            validate_reference(view, &node.key, anchor, "Anchor")?;
            if matches!(
                kind,
                OverlayKind::Popover
                    | OverlayKind::Tooltip
                    | OverlayKind::Select
                    | OverlayKind::Combobox
                    | OverlayKind::DropdownMenu
                    | OverlayKind::ContextMenu
            ) && view.nodes[anchor].kind == Kind::Overlay
            {
                anchor_edges.insert(node.key.as_str(), anchor);
            }
        }
        let needs_anchor = matches!(
            kind,
            OverlayKind::Popover
                | OverlayKind::Tooltip
                | OverlayKind::Select
                | OverlayKind::Combobox
                | OverlayKind::DropdownMenu
                | OverlayKind::ContextMenu
        );
        if needs_anchor && node.anchor.is_none() {
            return Err("anchored overlays require Anchor".into());
        }
        if node.open_request.is_some()
            && !matches!(kind, OverlayKind::Tooltip | OverlayKind::ContextMenu)
        {
            return Err("OpenRequest is valid only on Tooltip or ContextMenu".into());
        }
        if kind == OverlayKind::Tooltip {
            if node.label.trim().is_empty() && node.content.trim().is_empty() {
                return Err("Tooltip requires a description".into());
            }
            if node
                .children
                .iter()
                .any(|key| subtree_has_focusable(view, key))
            {
                return Err("Tooltip cannot contain focusable content".into());
            }
            let trigger = node
                .anchor
                .as_deref()
                .expect("Tooltip anchor checked above");
            if view.nodes.values().any(|candidate| {
                candidate.described_by.as_deref() == Some(node.key.as_str())
                    && candidate.key != trigger
            }) {
                return Err("Tooltip description must reference its anchor".into());
            }
        }
    }
    validate_anchor_cycles(&anchor_edges)?;
    validate_item_owners(view)?;
    Ok(())
}

fn validate_reference(
    view: &View,
    owner: &str,
    reference: &str,
    property: &str,
) -> Result<(), Error> {
    if reference.is_empty() || owner == reference || !view.nodes.contains_key(reference) {
        return Err(Error(format!(
            "{property} must reference another node in the view"
        )));
    }
    Ok(())
}

fn subtree_has_focusable(view: &View, key: &str) -> bool {
    let Some(node) = view.nodes.get(key) else {
        return false;
    };
    node.focusable()
        || node
            .children
            .iter()
            .any(|child| subtree_has_focusable(view, child))
}

fn validate_anchor_cycles(edges: &BTreeMap<&str, &str>) -> Result<(), Error> {
    fn visit<'a>(
        key: &'a str,
        edges: &BTreeMap<&'a str, &'a str>,
        active: &mut BTreeSet<&'a str>,
        done: &mut BTreeSet<&'a str>,
    ) -> Result<(), Error> {
        if done.contains(key) {
            return Ok(());
        }
        if !active.insert(key) {
            return Err("cyclic overlay anchors".into());
        }
        if let Some(next) = edges.get(key) {
            visit(next, edges, active, done)?;
        }
        active.remove(key);
        done.insert(key);
        Ok(())
    }
    let mut active = BTreeSet::new();
    let mut done = BTreeSet::new();
    for key in edges.keys() {
        visit(key, edges, &mut active, &mut done)?;
    }
    Ok(())
}

fn validate_item_owners(view: &View) -> Result<(), Error> {
    let mut owned = BTreeSet::new();
    for owner in view.nodes.values().filter(|node| node.kind.container()) {
        let mut values = BTreeSet::new();
        for key in &owner.children {
            let Some(item) = view.nodes.get(key) else {
                continue;
            };
            if !matches!(item.kind, Kind::MenuItem | Kind::OptionItem) {
                continue;
            }
            owned.insert(item.key.as_str());
            if item.option.trim().is_empty() || item.label.trim().is_empty() {
                return Err("menu and option items require nonempty Option and Label".into());
            }
            if !values.insert(item.option.as_str()) {
                return Err("menu and option values must be unique within their owner".into());
            }
            if item.kind == Kind::MenuItem && item.activate.is_none() {
                return Err("MenuItem requires Activate".into());
            }
            if item.kind == Kind::OptionItem && item.change.is_none() {
                return Err("OptionItem requires Change".into());
            }
        }
    }
    if view.nodes.values().any(|node| {
        matches!(node.kind, Kind::MenuItem | Kind::OptionItem) && !owned.contains(node.key.as_str())
    }) {
        return Err("menu and option items require a container owner".into());
    }
    Ok(())
}
