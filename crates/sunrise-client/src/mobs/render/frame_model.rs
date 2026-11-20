//! Immutable hierarchy and pose keys for the render model selected for a draw.

use super::super::{MobType, Part, anim};
use crate::fast_hash::FxHashMap;
use std::sync::Arc;

/// One part of a baked frame model. Links are indices into that model's nodes.
pub struct BakedFrameNode {
    pub name: Arc<str>,
    pub anim_key: u64,
    pub parent: Option<usize>,
    pub first_child: Option<usize>,
    pub next_sibling: Option<usize>,
}

/// A final model tree flattened in preorder, with root at index zero.
pub struct BakedFrameModel {
    nodes: Vec<BakedFrameNode>,
    by_name: FxHashMap<Arc<str>, usize>,
}

impl BakedFrameModel {
    pub(in crate::mobs) fn bake(root: &Part) -> Self {
        let mut layout = Self {
            nodes: Vec::new(),
            by_name: FxHashMap::default(),
        };
        layout.append(root, None);
        layout
    }

    fn append(&mut self, part: &Part, parent: Option<usize>) -> usize {
        let index = self.nodes.len();
        let name: Arc<str> = part.name.as_str().into();
        // Like ModelPart.createPartLookup, the first visit to a name wins.
        self.by_name.entry(name.clone()).or_insert(index);
        self.nodes.push(BakedFrameNode {
            name,
            anim_key: anim::key(&part.name),
            parent,
            first_child: None,
            next_sibling: None,
        });
        let mut previous: Option<usize> = None;
        for child in &part.children {
            let child = self.append(child, Some(index));
            if let Some(previous) = previous {
                self.nodes[previous].next_sibling = Some(child);
            } else {
                self.nodes[index].first_child = Some(child);
            }
            previous = Some(child);
        }
        index
    }

    pub fn nodes(&self) -> &[BakedFrameNode] {
        &self.nodes
    }

    /// The first depth-first match for this exact name, including the root.
    pub fn find(&self, name: &str) -> Option<usize> {
        self.by_name.get(name).copied()
    }
}

/// Layouts for the vanilla mob and player models held by one set of client resources.
#[derive(Default)]
pub struct BakedFrameModels {
    // Retain each root so its address cannot be reused by a different model.
    by_model: FxHashMap<usize, (Arc<Part>, BakedFrameModel)>,
}

impl BakedFrameModels {
    /// Bake each type's default and all its layers, once per shared model root.
    pub fn bake<'a>(types: impl IntoIterator<Item = &'a MobType>) -> Self {
        let mut layouts = Self::default();
        for kind in types {
            for model in std::iter::once(&kind.model).chain(kind.layers.values()) {
                layouts
                    .by_model
                    .entry(Arc::as_ptr(model) as usize)
                    .or_insert_with(|| (model.clone(), BakedFrameModel::bake(model)));
            }
        }
        layouts
    }

    /// Find the layout for the exact tree selected by the renderer.
    pub fn get(&self, model: &Arc<Part>) -> Option<&BakedFrameModel> {
        self.by_model
            .get(&(Arc::as_ptr(model) as usize))
            .map(|(_, layout)| layout)
    }
}
