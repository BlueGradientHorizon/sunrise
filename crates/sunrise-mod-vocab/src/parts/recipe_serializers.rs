//! Read-only serializers share registration and native values with crafting serializers.
use super::*;
/// A serializer for a separately registered, mod-read recipe type.
/// The engine publishes its source JSON through recipes_of_type and never matches it.
pub trait RecipeSerializer: Send + Sync + 'static {
    /// Mod-owned recipe data retained with the serializer for the loaded generation.
    type Value: Send + Sync + 'static;
    /// Decode the JSON object without type; a refusal skips this known recipe alone.
    fn decode(&self, json: &str) -> Result<Self::Value, String>;
    /// Write the decoded value for synchronization.
    fn write(&self, value: &Self::Value) -> Result<String, String>;
}
struct ReadOnly<S>(S);
impl<S: RecipeSerializer> crate::CraftingRecipeKind for ReadOnly<S> {
    type Value = S::Value;
    fn decode(&self, json: &str) -> Result<Self::Value, String> {
        self.0.decode(json)
    }
    fn write(&self, value: &Self::Value) -> Result<String, String> {
        self.0.write(value)
    }
    fn matches(&self, _: &dyn crate::LoopContext, _: &Self::Value, _: crate::CraftingGrid) -> bool {
        false
    }
    fn assemble(
        &self,
        _: &dyn crate::LoopContext,
        _: &Self::Value,
        _: crate::CraftingGrid,
    ) -> Option<crate::ItemStack> {
        None
    }
}
struct Raw;
impl RecipeSerializer for Raw {
    type Value = String;
    fn decode(&self, json: &str) -> Result<String, String> {
        // The loader supplies a compact object with `type` removed. Keep the old
        // string limit: 1 MiB minus the four bytes of its former length prefix.
        const MAX_RAW_RECIPE_JSON_BYTES: usize = 1024 * 1024 - 4;
        if json.len() > MAX_RAW_RECIPE_JSON_BYTES {
            return Err(format!(
                "raw recipe JSON exceeds {MAX_RAW_RECIPE_JSON_BYTES} bytes"
            ));
        }
        Ok(json.to_owned())
    }
    fn write(&self, value: &String) -> Result<String, String> {
        Ok(value.clone())
    }
}
impl dyn Registrar + '_ {
    /// Register a serializer for an independently declared read-only recipe type.
    pub fn register_read_only_recipe_serializer<S: RecipeSerializer>(
        &mut self,
        name: &str,
        recipe_type: &str,
        serializer: S,
    ) {
        let kind = ReadOnly(serializer);
        let handler = crate::parts::crafting_recipes::typed_handler(kind);
        self.add_recipe_serializer(name, recipe_type, handler);
    }
    /// Register an object-preserving JSON serializer for a mod-read machine recipe type.
    /// Declare recipe_type with register_recipe_type first (or elsewhere at startup).
    pub fn register_raw_recipe_serializer(&mut self, name: &str, recipe_type: &str) {
        self.register_read_only_recipe_serializer(name, recipe_type, Raw);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_recipe_accepts_the_previous_json_limit_but_not_one_byte_more() {
        let padding = "x".repeat(1_048_572 - r#"{"value":""}"#.len());
        let at_limit = format!(r#"{{"value":"{padding}"}}"#);
        assert_eq!(Raw.decode(&at_limit).unwrap(), at_limit);
        let over_limit = format!(r#"{{"value":"{padding}x"}}"#);
        assert!(Raw.decode(&over_limit).is_err());
    }
}
