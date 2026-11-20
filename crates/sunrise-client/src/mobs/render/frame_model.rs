//! Immutable hierarchy and pose keys for the render model selected for a draw.

use super::super::{MobType, Part, anim};
use crate::fast_hash::FxHashMap;
use mod_api::{FrameModel, FrameModelPart};
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
    by_model: FxHashMap<usize, (Arc<Part>, Arc<BakedFrameModel>)>,
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
                    .or_insert_with(|| (model.clone(), Arc::new(BakedFrameModel::bake(model))));
            }
        }
        layouts
    }

    /// Find the layout for the exact tree selected by the renderer.
    pub fn get(&self, model: &Arc<Part>) -> Option<&Arc<BakedFrameModel>> {
        self.by_model
            .get(&(Arc::as_ptr(model) as usize))
            .map(|(_, layout)| layout)
    }
}

/// The exact body poses kept alive for layer and Post callbacks. Dropping this returns the
/// poses to the render thread's pool. The shared layout also outlives the CEM read guard.
pub struct FrameModelPose {
    pub(super) layout: Arc<BakedFrameModel>,
    pub(super) poses: anim::PooledPoses,
}

impl FrameModelPose {
    fn view<'a>(&'a self, index: usize, node: &'a BakedFrameNode) -> FrameModelPart<'a> {
        let mut ancestor = Some(node);
        let mut visible = true;
        while let Some(current) = ancestor {
            if self.poses.is_hidden_by_key(current.anim_key) {
                visible = false;
                break;
            }
            ancestor = current.parent.map(|i| &self.layout.nodes[i]);
        }
        FrameModelPart {
            index,
            name: &node.name,
            parent: node.parent,
            first_child: node.first_child,
            next_sibling: node.next_sibling,
            local_pose: self
                .poses
                .pose_by_key(node.anim_key)
                .expect("poses were reset from the baked model's tree"),
            visible,
        }
    }
}

impl FrameModel for FrameModelPose {
    fn root(&self) -> FrameModelPart<'_> {
        self.view(0, &self.layout.nodes[0])
    }

    fn part(&self, name: &str) -> Option<FrameModelPart<'_>> {
        self.part_at(self.layout.find(name)?)
    }

    fn part_at(&self, index: usize) -> Option<FrameModelPart<'_>> {
        Some(self.view(index, self.layout.nodes.get(index)?))
    }
}
