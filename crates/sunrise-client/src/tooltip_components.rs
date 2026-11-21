//! Client tooltip component factories (`RegisterClientTooltipComponentFactoriesEvent`,
//! NeoForge's `ClientTooltipComponentManager`): maps custom [`mod_api::TooltipComponentData`] types
//! to [`PreparedTooltipComponent`]s once at client startup.

use mod_api::{
    __private::game::CallbackOwner, ClientTooltipComponentFactoriesRegistrar,
    PreparedTooltipComponent, TooltipComponent, TooltipComponentFactory,
    content::crash::current_native_mod,
};
#[cfg(any(test, feature = "test-tooltip-factories"))]
use std::cell::RefCell;
use std::{any::TypeId, collections::HashMap, sync::OnceLock};

#[derive(Clone)]
struct RegisteredFactory {
    owner: Option<CallbackOwner>,
    factory: TooltipComponentFactory,
}

/// Client startup registrar collecting custom tooltip component factories before freeze.
#[derive(Default)]
pub struct TooltipComponentFactories {
    factories: HashMap<TypeId, RegisteredFactory>,
}

impl ClientTooltipComponentFactoriesRegistrar for TooltipComponentFactories {
    fn register_factory(&mut self, factory: TooltipComponentFactory) {
        let owner = current_native_mod().map(CallbackOwner::for_mod);
        self.factories
            .insert(factory.__type_id(), RegisteredFactory { owner, factory });
    }
}

/// Frozen lookup table of client tooltip component factories published at client startup.
pub struct FrozenTooltipComponentFactories {
    factories: HashMap<TypeId, RegisteredFactory>,
}

static FACTORIES: OnceLock<FrozenTooltipComponentFactories> = OnceLock::new();

#[cfg(any(test, feature = "test-tooltip-factories"))]
thread_local! {
    static TEST_FACTORIES: RefCell<Option<FrozenTooltipComponentFactories>> = const { RefCell::new(None) };
}

/// Publish the client startup tooltip component factory table once, before any tooltip renders.
pub fn freeze(factories: TooltipComponentFactories) {
    if FACTORIES
        .set(FrozenTooltipComponentFactories {
            factories: factories.factories,
        })
        .is_err()
    {
        log::warn!("tooltip component factories were registered already; these are left out");
    }
}

/// Install scoped tooltip component factories on this thread for the duration of `f`, restoring
/// the previous value on return or unwind.
#[cfg(any(test, feature = "test-tooltip-factories"))]
pub fn with_test_factories<R>(factories: TooltipComponentFactories, f: impl FnOnce() -> R) -> R {
    struct Restore(Option<FrozenTooltipComponentFactories>);
    impl Drop for Restore {
        fn drop(&mut self) {
            TEST_FACTORIES.with(|slot| {
                *slot.borrow_mut() = self.0.take();
            });
        }
    }
    let previous = TEST_FACTORIES.with(|slot| {
        slot.replace(Some(FrozenTooltipComponentFactories {
            factories: factories.factories,
        }))
    });
    let _guard = Restore(previous);
    f()
}

/// Prepare a custom [`TooltipComponent`] using the registered client factory for its data type.
///
/// Built-in bundle components return `None` here (the tooltip pipeline prepares them directly
/// with `Items`). Unregistered custom types log a warning and return `None` without panicking.
pub fn create(component: &TooltipComponent) -> Option<PreparedTooltipComponent> {
    if component.as_bundle().is_some() {
        return None;
    }
    let (Some(type_id), Some(data)) = (component.type_id(), component.__as_any()) else {
        return None;
    };
    #[cfg(any(test, feature = "test-tooltip-factories"))]
    if let Some(found) = TEST_FACTORIES.with(|slot| {
        slot.borrow()
            .as_ref()
            .map(|frozen| frozen.factories.get(&type_id).cloned())
    }) {
        return match found {
            Some(entry) => entry.factory.__create(data, entry.owner),
            None => {
                log::warn!(
                    "no client tooltip component factory registered for {}",
                    component.type_name().unwrap_or("<unknown>")
                );
                None
            }
        };
    }
    let Some(entry) = FACTORIES
        .get()
        .and_then(|frozen| frozen.factories.get(&type_id))
    else {
        log::warn!(
            "no client tooltip component factory registered for {}",
            component.type_name().unwrap_or("<unknown>")
        );
        return None;
    };
    entry.factory.__create(data, entry.owner.clone())
}

/// Whether any custom tooltip component factory is registered in the active table.
pub fn has_factories() -> bool {
    #[cfg(any(test, feature = "test-tooltip-factories"))]
    if let Some(has) = TEST_FACTORIES.with(|slot| {
        slot.borrow()
            .as_ref()
            .map(|frozen| !frozen.factories.is_empty())
    }) {
        return has;
    }
    FACTORIES
        .get()
        .is_some_and(|frozen| !frozen.factories.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use mod_api::{
        ClientTooltipComponent, Identifier, Text, TextFormError, TooltipComponentData, TooltipFont,
    };

    struct MetricData(i32);
    impl TooltipComponentData for MetricData {}

    struct UnregisteredData;
    impl TooltipComponentData for UnregisteredData {}

    struct MetricClient {
        width: i32,
        height: i32,
    }
    impl ClientTooltipComponent for MetricClient {
        fn height(&self, _font: &dyn TooltipFont) -> i32 {
            self.height
        }
        fn width(&self, _font: &dyn TooltipFont) -> i32 {
            self.width
        }
    }

    struct StubFont(Identifier);
    impl TooltipFont for StubFont {
        fn id(&self) -> &Identifier {
            &self.0
        }
        fn line_height(&self) -> i32 {
            9
        }
        fn width(&self, _text: &Text) -> Result<i32, TextFormError> {
            Ok(0)
        }
        fn split(&self, _text: &Text, _max_width: i32) -> Result<Vec<Text>, TextFormError> {
            Ok(Vec::new())
        }
    }

    #[test]
    fn factories_create_replace_warn_unregistered_and_restore_on_unwind() {
        let font = StubFont(Identifier::parse("minecraft:default").unwrap());
        let data = TooltipComponent::custom(MetricData(12));
        let unknown = TooltipComponent::custom(UnregisteredData);

        with_test_factories(TooltipComponentFactories::default(), || {
            assert!(!has_factories());
            assert!(create(&data).is_none());

            let mut factories = TooltipComponentFactories::default();
            let registrar: &mut dyn ClientTooltipComponentFactoriesRegistrar = &mut factories;
            registrar.register::<MetricData, MetricClient>(|d| MetricClient {
                width: d.0,
                height: 10,
            });
            // Last registration wins, matching NeoForge HashMap::put.
            registrar.register::<MetricData, MetricClient>(|d| MetricClient {
                width: d.0 * 3,
                height: 20,
            });

            with_test_factories(factories, || {
                assert!(has_factories());
                let prepared = create(&data).expect("registered factory should prepare component");
                assert_eq!(prepared.width(&font), 36);
                assert_eq!(prepared.height(&font), 20);
                assert!(create(&unknown).is_none());
            });

            // Inner scope restored outer empty table.
            assert!(!has_factories());
            assert!(create(&data).is_none());

            // Unwind safety: a panic inside with_test_factories still restores the prior table.
            let mut inner = TooltipComponentFactories::default();
            let registrar: &mut dyn ClientTooltipComponentFactoriesRegistrar = &mut inner;
            registrar.register::<MetricData, MetricClient>(|d| MetricClient {
                width: d.0,
                height: 1,
            });
            let panicked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                with_test_factories(inner, || {
                    assert!(has_factories());
                    panic!("expected unwind");
                });
            }));
            assert!(panicked.is_err());
            assert!(!has_factories());
        });
    }
}
