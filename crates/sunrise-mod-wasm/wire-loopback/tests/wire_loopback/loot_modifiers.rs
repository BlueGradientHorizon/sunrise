use super::*;
use mod_api::{
    DataLoadContext, GlobalLootModifier, GlobalLootModifierSerializer, Identifier,
    LegacyRandomSource, LootContext, LootModifierContext, LootModifierError, LootModifierSettings,
    LootStack, RandomSource,
};

struct Codec;
impl GlobalLootModifierSerializer for Codec {
    fn decode(
        &self,
        data: &serde_json::Value,
        load: &dyn DataLoadContext,
    ) -> Result<Arc<dyn GlobalLootModifier>, LootModifierError> {
        assert_eq!(load.origin(), mod_api::DataLoadOrigin::ServerDataLoad);
        Ok(Arc::new(Modifier(LootModifierSettings::decode(data)?)))
    }
}
struct Modifier(LootModifierSettings);
impl GlobalLootModifier for Modifier {
    fn priority(&self) -> Result<i32, LootModifierError> {
        Ok(self.0.priority)
    }
    fn condition(&self) -> Result<Option<serde_json::Value>, LootModifierError> {
        Ok(self.0.condition.clone())
    }
    fn apply(
        &self,
        context: &mut dyn LootModifierContext,
        mut generated: Vec<LootStack>,
    ) -> Result<Vec<LootStack>, LootModifierError> {
        assert_eq!(context.snapshot().queried_table_id, "test:root");
        generated.reverse();
        generated[1].count -= context.random().next_int_bound(2);
        generated.extend(context.roll_raw(&Identifier::parse("test:raw").unwrap())?);
        Ok(generated)
    }
}
struct Roll(LegacyRandomSource);
impl RandomSource for Roll {
    fn next_int(&mut self) -> i32 {
        self.0.next_int()
    }
    fn next_int_bound(&mut self, bound: i32) -> i32 {
        self.0.next_int_bound(bound)
    }
    fn next_long(&mut self) -> i64 {
        self.0.next_long()
    }
    fn next_float(&mut self) -> f32 {
        self.0.next_float()
    }
    fn next_double(&mut self) -> f64 {
        self.0.next_double()
    }
    fn next_boolean(&mut self) -> bool {
        self.0.next_boolean()
    }
    fn next_gaussian(&mut self) -> f64 {
        self.0.next_gaussian()
    }
}
impl LootModifierContext for Roll {
    fn snapshot(&self) -> LootContext {
        let mut snapshot = LootContext::new(0);
        snapshot.queried_table_id = "test:root".into();
        snapshot
    }
    fn random(&mut self) -> &mut dyn RandomSource {
        self
    }
    fn roll_raw(&mut self, table: &Identifier) -> Result<Vec<LootStack>, LootModifierError> {
        assert_eq!(table.to_string(), "test:raw");
        Ok(vec![LootStack::new(
            REGISTERED_ITEM.get().unwrap(),
            self.random().next_int_bound(5),
        )])
    }
}
struct Reader;
impl mod_api::LoadReader for Reader {
    fn registry(&self, _: &str) -> Option<Arc<[String]>> {
        None
    }
    fn fluid_tags(&self) -> Arc<[(String, Arc<[String]>)]> {
        Arc::from([])
    }
    fn setting(&self, _: &str, _: &str) -> Option<String> {
        None
    }
}

#[test]
fn native_and_guest_prepared_modifiers_lend_the_same_roll_random_and_raw_context() {
    mod_api::__private::initialize().unwrap();
    let _phase = mod_api::__private::PhaseOwner::scope(mod_api::__private::ModPhase::World);
    install_counter();
    for wasm in [false, true] {
        let serializer: Arc<dyn GlobalLootModifierSerializer> = if wasm {
            let index = guest::add_handler(vec![guest::view::<dyn GlobalLootModifierSerializer>(
                Arc::new(Codec),
            )]);
            Arc::new(WasmHandler {
                index,
                module: WasmMod {
                    group: 0,
                    subscribed: &[],
                    listeners: &[],
                    corrupt: false,
                },
            })
        } else {
            Arc::new(Codec)
        };
        let load = mod_api::OwningLoad::new(mod_api::DataLoadOrigin::ServerDataLoad, &Reader);
        let instance = serializer
            .decode(
                &serde_json::json!({"priority":-37,"condition":"test:predicate"}),
                &load,
            )
            .unwrap();
        drop(serializer);
        assert_eq!(instance.priority().unwrap(), -37);
        assert_eq!(
            instance.condition().unwrap(),
            Some(serde_json::json!("test:predicate"))
        );
        let mut roll = Roll(LegacyRandomSource::new(0));
        let mut expected = LegacyRandomSource::new(0);
        let count = -7 - expected.next_int_bound(2);
        let raw = expected.next_int_bound(5);
        let result = instance
            .apply(
                &mut roll,
                vec![
                    LootStack::new(REGISTERED_ITEM.get().unwrap(), -7),
                    LootStack::new(REGISTERED_ITEM.get().unwrap(), 130),
                ],
            )
            .unwrap();
        assert_eq!(
            result,
            vec![
                LootStack::new(REGISTERED_ITEM.get().unwrap(), 130),
                LootStack::new(REGISTERED_ITEM.get().unwrap(), count),
                LootStack::new(REGISTERED_ITEM.get().unwrap(), raw)
            ]
        );
        assert_eq!(roll.next_long(), expected.next_long());
    }
}
