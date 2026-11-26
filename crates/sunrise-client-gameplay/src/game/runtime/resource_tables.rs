#[cfg(feature = "client")]
#[cfg(any(test, feature = "test-support"))]
thread_local! {
    pub(in crate::game) static DYNAMIC_TEXTURE_POOL_OVERRIDE: std::cell::Cell<Option<bool>> =
        const { std::cell::Cell::new(None) };
}

#[cfg(feature = "client")]
#[cfg(any(test, feature = "test-support"))]
pub fn with_dynamic_texture_pool_override<R>(pool: bool, f: impl FnOnce() -> R) -> R {
    struct Restore(Option<bool>);
    impl Drop for Restore {
        fn drop(&mut self) {
            DYNAMIC_TEXTURE_POOL_OVERRIDE.set(self.0);
        }
    }
    let _restore = Restore(DYNAMIC_TEXTURE_POOL_OVERRIDE.replace(Some(pool)));
    f()
}

#[cfg(feature = "client")]
#[cfg(any(test, feature = "test-support"))]
impl crate::game::client_side::resource_tables_client::UiResources {
    pub fn clone_for_tests(&self) -> Self {
        use crate::game::client_side::resource_tables_client::PlainTexts;
        Self {
            atlas: self.atlas.clone_for_tests(),
            panorama: self.panorama.clone(),
            splashes: self.splashes.clone(),
            plain_texts: PlainTexts::default(),
        }
    }
}

#[cfg(feature = "client")]
#[cfg(test)]
pub(in crate::game) mod plain_text_tests {
    use crate::game::client_side::resource_tables_client::PlainTexts;

    #[test]
    fn a_string_drawn_again_reuses_its_text_and_the_store_stays_bounded() {
        let texts = PlainTexts::default();
        let first = texts.with("Jump", |t| t as *const _);
        assert_eq!(texts.with("Jump", |t| t as *const _), first);
        assert_eq!(texts.with("Jump", |t| t.plain()), "Jump");
        for i in 0..PlainTexts::MAX * 2 {
            texts.with(&i.to_string(), |_| ());
        }
        assert!(texts.0.borrow().len() <= PlainTexts::MAX);
    }
}
