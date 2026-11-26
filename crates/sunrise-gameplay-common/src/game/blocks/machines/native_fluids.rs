

#[cfg(any(test, feature = "test-support"))]
pub mod tests {

use super::*;

use mod_api::ServerHost;

/// Vanilla's table, with an empty cauldron, water at `level` and lava on it: what the
    /// cauldron holds as (water level, lava) for the assertions.
    pub fn vanilla_cauldrons() -> (
        CauldronFluids,
        impl Fn(u32) -> CauldronFluids,
        CauldronFluids,
        impl Fn(&CauldronFluids) -> (u32, bool),
    ) {
        let contents = Arc::new(CauldronContents::vanilla());
        let water = contents.of_block("minecraft:water_cauldron").unwrap().0;
        let lava = contents.of_block("minecraft:lava_cauldron").unwrap().0;
        let empty = CauldronFluids {
            contents,
            held: None,
        };
        let level = move |cauldron: &CauldronFluids| match cauldron.held {
            None => (0, false),
            Some((index, level)) if index == water => (level, false),
            Some(_) => (0, true),
        };
        (
            empty.clone(),
            {
                let empty = empty.clone();
                move |level| empty.with(Some((water, level)))
            },
            empty.with(Some((lava, 1))),
            level,
        )
    }
#[cfg(test)]
impl mod_api::ResourceHandler<mod_api::FluidResource> for WaterTank {
        fn size(&self, _: &mut dyn ServerHost) -> usize {
            self.reached.len()
        }
        fn resource(
            &self,
            host: &mut dyn ServerHost,
            index: usize,
        ) -> Option<mod_api::FluidResource> {
            let real = *self.reached.get(index)?;
            let (fluid, amount) = self.state(host).swap_remove(real);
            fluid.filter(|_| amount > 0).map(|fluid| {
                mod_api::FluidResource::of(&FluidStack::new(
                    mod_api::__private::fluid_key(&fluid).unwrap(),
                    1,
                ))
            })
        }
        fn amount(&self, host: &mut dyn ServerHost, index: usize) -> u32 {
            self.reached
                .get(index)
                .map_or(0, |&real| self.state(host)[real].1.max(0).unsigned_abs())
        }
        fn capacity(
            &self,
            _: &mut dyn ServerHost,
            index: usize,
            _: Option<&mod_api::FluidResource>,
        ) -> u32 {
            if index < self.reached.len() { 4000 } else { 0 }
        }
        fn is_valid(
            &self,
            _: &mut dyn ServerHost,
            index: usize,
            resource: &mod_api::FluidResource,
        ) -> bool {
            index < self.reached.len()
                && resource
                    == &mod_api::FluidResource::of(&FluidStack::new(
                        mod_api::__private::fluid_key("minecraft:water").unwrap(),
                        1,
                    ))
        }
        fn insert_at(
            &self,
            host: &mut dyn ServerHost,
            _: &TransactionContext,
            index: usize,
            resource: &mod_api::FluidResource,
            amount: u32,
        ) -> Result<u32, mod_api::TransactionError<mod_api::Overanswered>> {
            if !self.is_valid(host, index, resource) {
                return Ok(0);
            }
            let mut state = self.state(host);
            let entry = &mut state[self.reached[index]];
            if entry.1 > 0 && entry.0.as_deref() != Some("minecraft:water") {
                return Ok(0);
            }
            let took = amount.min((4000 - entry.1).max(0).unsigned_abs());
            if took == 0 {
                return Ok(0);
            }
            entry.0 = Some("minecraft:water".into());
            entry.1 += took as i32;
            Ok(if self.write(host, &state) { took } else { 0 })
        }
        fn extract_at(
            &self,
            host: &mut dyn ServerHost,
            _: &TransactionContext,
            index: usize,
            resource: &mod_api::FluidResource,
            amount: u32,
        ) -> Result<u32, mod_api::TransactionError<mod_api::Overanswered>> {
            let Some(&real) = self.reached.get(index) else {
                return Ok(0);
            };
            if self.resource(host, index).as_ref() != Some(resource) {
                return Ok(0);
            }
            let mut state = self.state(host);
            let entry = &mut state[real];
            let took = amount.min(entry.1.max(0).unsigned_abs());
            if took == 0 {
                return Ok(0);
            }
            entry.1 -= took as i32;
            if entry.1 == 0 {
                entry.0 = None;
            }
            Ok(if self.write(host, &state) { took } else { 0 })
        }
    }
pub fn tanks(list: &[(Option<&str>, i32)]) -> Vec<Tank> {
        list.iter()
            .map(|&(fluid, amount)| (fluid.map(str::to_owned), amount))
            .collect()
    }
}
