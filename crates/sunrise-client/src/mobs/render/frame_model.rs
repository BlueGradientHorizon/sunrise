//! A draw's selected body, baked hierarchy, and captured pose.

use super::{Atlas, body, cem_key, cem_texture};
use crate::fast_hash::FxHashMap;
use crate::mobs::{Mob, MobData, MobType, Part, anim, cem};
use mod_api::{FrameModel, FrameModelPart};
use std::{borrow::Cow, sync::Arc};

/// The exact body selected for one draw, borrowing the mob and an owned pack snapshot.
/// A caller can keep this selection across Pre without holding the pack's read lock.
pub(super) struct FrameBody<'a> {
    model_key: Cow<'static, str>,
    pub(super) model: &'a Arc<Part>,
    pub(super) custom: Option<&'a cem::CemLayer>,
    pub(super) texture: Cow<'a, str>,
}

impl<'a> FrameBody<'a> {
    pub(super) fn select(mob: &'a Mob, cem: &'a cem::CemModels, atlas: Atlas<'_>) -> Self {
        let (model_key, texture) = body(mob);
        let custom = cem.layer(cem_key(&model_key));
        let model = custom
            .map(|layer| &layer.model)
            .unwrap_or_else(|| mob.kind.layer(&model_key).unwrap_or(&mob.kind.model));
        let texture = cem_texture(custom, &texture, atlas).map_or(texture, Cow::Borrowed);
        Self {
            model_key,
            model,
            custom,
            texture,
        }
    }

    /// Both names of the body, so overlays on it don't take the `.jem` texture.
    pub(super) fn keys(&self) -> [&str; 2] {
        [&self.model_key, cem_key(&self.model_key)]
    }

    pub(super) fn layout(&self, data: &'a MobData) -> Option<&'a Arc<BakedFrameModel>> {
        self.custom
            .map(|layer| &layer.frame_model)
            .or_else(|| data.frame_models.get(self.model))
    }
}

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
/// poses to the render thread's pool. The shared layout also outlives the pack snapshot.
pub struct FrameModelPose {
    pub(super) layout: Arc<BakedFrameModel>,
    pub(super) poses: anim::PooledPoses,
}

impl FrameModelPose {
    fn view<'a>(&'a self, index: usize, node: &'a BakedFrameNode) -> FrameModelPart<'a> {
        let mut ancestor = self.poses.has_hidden_parts().then_some(node);
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
