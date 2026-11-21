//! Authoritative placement data crosses one checked load boundary, never a draw path.
#[cfg(test)]
#[path = "mod_placement_tests.rs"]
mod tests;
use super::{DisplayLoadError, DisplayResources, RecipeBookCategory};
use crate::{
    crafting::{Ingredient, Placement},
    menu::place::custom_plan::{MenuPlacement, PlacementRefusal},
};
use mod_api::{PlacementInfo, RecipeBookDisplayEntry, item::crafting::Shape};

fn invalid(reason: impl Into<String>) -> DisplayLoadError {
    DisplayLoadError::InvalidPlacement(reason.into())
}

pub(super) fn validate_batch(
    values: &[RecipeBookCategory],
    resources: &dyn DisplayResources,
) -> Result<(), DisplayLoadError> {
    for category in values {
        for entry in &category.displays {
            let Some(info) = &entry.placement else {
                continue;
            };
            validate(info, resources).map_err(|reason| {
                invalid(format!("recipe {} placement: {reason}", entry.recipe))
            })?;
        }
    }
    Ok(())
}
fn validate(info: &PlacementInfo, resources: &dyn DisplayResources) -> Result<(), String> {
    if let Some([w, h]) = info.shape
        && (w == 0
            || h == 0
            || usize::from(w).checked_mul(usize::from(h)) != Some(info.cells.len()))
    {
        return Err("shape dimensions disagree with cells".into());
    }
    let mut pending: Vec<_> = info.cells.iter().flatten().collect();
    while let Some(ingredient) = pending.pop() {
        if ingredient.items.iter().any(|id| !resources.item(id)) {
            return Err("unknown ingredient item".into());
        }
        let ordered: std::collections::HashSet<_> = ingredient.order.iter().cloned().collect();
        if !ordered.is_subset(&ingredient.items)
            || (simple_ingredient(ingredient) && ordered != ingredient.items)
            || ordered.len() != ingredient.order.len()
        {
            return Err("ingredient holder order disagrees with its items".into());
        }
        if ingredient.custom.is_some()
            && (ingredient.children.is_some() || ingredient.rule.is_some())
        {
            return Err("custom ingredient cannot also have a child shape or built-in rule".into());
        }
        if let Some((shape, children)) = &ingredient.children {
            let valid = match shape {
                Shape::Compound | Shape::Intersection => !children.is_empty(),
                Shape::Difference => children.len() == 2,
                Shape::CustomDisplay => children.len() == 1,
            };
            if !valid {
                return Err("ingredient shape has invalid children".into());
            }
            pending.extend(children);
        }
    }
    Ok(())
}

pub(super) fn compile(
    owner: &str,
    menu: &str,
    entry: &RecipeBookDisplayEntry,
    layout: &super::super::slot_layout::CheckedSlotLayout,
) -> Result<Option<MenuPlacement>, DisplayLoadError> {
    let info = entry.placement.as_ref().ok_or_else(|| {
        invalid(format!(
            "mod {owner} recipe {}: menu-bound recipe type must supply PlacementInfo",
            entry.recipe
        ))
    })?;
    let cells = info
        .cells
        .iter()
        .map(|cell| cell.as_ref().map(convert))
        .collect();
    let placement = Placement::of(
        cells,
        info.shape.map(|[w, h]| (usize::from(w), usize::from(h))),
    );
    let compiled = match layout.declaration() {
        mod_api::RecipeBookLayout::Mod => MenuPlacement::for_mod(menu.into(), placement),
        _ => MenuPlacement::new(menu.into(), placement, layout),
    };
    match compiled {
        Ok(plan) => Ok(Some(plan)),
        Err(PlacementRefusal::NotPlaceable) => Ok(None),
        Err(error) => Err(invalid(format!(
            "mod {owner} recipe {} placement: {error:?}",
            entry.recipe
        ))),
    }
}
// Only called after validate_batch accepted the native declaration.
fn convert(value: &mod_api::Ingredient) -> Ingredient {
    Ingredient {
        items: value.items.clone(),
        order: value.order.clone(),
        custom: value.custom.clone(),
        rule: None,
        compiled: None,
        display: value.display.as_deref().cloned(),
        component_defaults: Default::default(),
        shown: None,
        children: value
            .children
            .as_ref()
            .map(|(shape, children)| (shape.clone(), children.iter().map(convert).collect())),
    }
}

fn simple_ingredient(ingredient: &mod_api::Ingredient) -> bool {
    let mut pending = vec![ingredient];
    while let Some(ingredient) = pending.pop() {
        if ingredient.rule.is_some() || ingredient.custom.is_some() {
            return false;
        }
        if let Some((_, children)) = &ingredient.children {
            pending.extend(children);
        }
    }
    true
}
