//! Typed `ContextIntProvider` and `ContextFloatProvider` evaluation.
use super::{Providers, Value};
use std::collections::HashMap;
use sunrise_core::{
    java_number::float_to_string,
    random::{LevelRandom, RandomSource},
};
use sunrise_items::item_data::cooking_fuel::{CookingFuel, Resolvable};

type Arith = String;

/// The same numeric evaluation for commands and container processes. The
/// caller supplies the level's random source and its loot-condition context.
pub struct Evaluation<'a, F> {
    pub providers: &'a Providers,
    pub random: &'a mut LevelRandom,
    pub condition: F,
}

fn lookup<'a>(table: &'a HashMap<String, Value>, id: &str) -> Option<&'a Value> {
    if let Some(path) = id.strip_prefix(':') {
        table.get(&format!("minecraft:{path}"))
    } else if id.contains(':') {
        table.get(id)
    } else {
        table.get(&format!("minecraft:{id}"))
    }
}

impl<F: FnMut(&Value, &mut LevelRandom) -> bool> Evaluation<'_, F> {
    /// `AbstractFurnaceBlockEntity.getBurnDuration` and `getSpeedMultiplier`.
    /// Missing keys use the field default; a present provider uses its safe
    /// getter, returning zero for arithmetic failures or non-finite float results.
    pub fn cooking_fuel(&mut self, fuel: &CookingFuel) -> (i32, f32) {
        let time = match &fuel.burn_time {
            Resolvable::Constant(value) => *value,
            Resolvable::Reference(key) => self
                .providers
                .ints
                .get(key)
                .map_or(0, |v| self.int(v, 0).unwrap_or(0)),
        };
        let speed = match &fuel.speed_multiplier {
            Resolvable::Constant(value) => *value,
            Resolvable::Reference(key) => self.providers.floats.get(key).map_or(1.0, |v| {
                self.float(v, 0)
                    .ok()
                    .filter(|v| v.is_finite())
                    .unwrap_or(0.0)
            }),
        };
        (time, speed)
    }

    /// `ContextIntProvider.getIntUnsafe`.
    pub fn int(&mut self, v: &Value, depth: u32) -> Result<i32, Arith> {
        if depth > 64 {
            return Ok(0);
        }
        let d = depth + 1;
        match v {
            Value::Number(n) => Ok(n
                .as_i64()
                .map_or_else(|| n.as_f64().unwrap_or(0.0) as i32, |i| i as i32)),
            Value::Bool(b) => Ok(i32::from(*b)),
            Value::String(id) => match lookup(&self.providers.ints, id) {
                Some(def) => self.int(def, d),
                None => Ok(0),
            },
            Value::Object(o) => {
                let ty = o
                    .get("type")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .trim_start_matches("minecraft:");
                let inputs = |g: &mut Self| -> Result<Vec<i32>, Arith> {
                    let list = match o.get("inputs") {
                        Some(Value::Array(list)) => list.as_slice(),
                        Some(other) => std::slice::from_ref(other),
                        None => &[],
                    };
                    list.iter().map(|x| g.int(x, d)).collect()
                };
                let field = |g: &mut Self, k: &str| -> Result<i32, Arith> {
                    match o.get(k) {
                        Some(x) => g.int(x, d),
                        None => Ok(0),
                    }
                };
                let overflow = || "integer overflow".to_owned();
                let long_to_int = |v: i64| -> Result<i32, Arith> {
                    i32::try_from(v)
                        .map_err(|_| format!("Value {v} can't be safely converted to int"))
                };
                Ok(match ty {
                    "constant" => field(self, "value")?,
                    "add" => long_to_int(inputs(self)?.iter().map(|&x| i64::from(x)).sum())?,
                    "mul" => long_to_int(
                        inputs(self)?
                            .iter()
                            .fold(1i64, |a, &x| a.wrapping_mul(i64::from(x))),
                    )?,
                    "avg" => {
                        let xs = inputs(self)?;
                        if xs.is_empty() {
                            return Err("/ by zero".into());
                        }
                        let sum: i64 = xs.iter().map(|&x| i64::from(x)).sum();
                        long_to_int(sum / xs.len() as i64)?
                    }
                    "max" => inputs(self)?.into_iter().fold(i32::MIN, i32::max),
                    "min" => inputs(self)?.into_iter().fold(i32::MAX, i32::min),
                    "sub" => {
                        let (a, b) = (field(self, "left")?, field(self, "right")?);
                        a.checked_sub(b).ok_or_else(overflow)?
                    }
                    "div" | "mod" | "floor_div" | "floor_mod" => {
                        let (a, b) = (field(self, "left")?, field(self, "right")?);
                        if b == 0 {
                            return Err("/ by zero".into());
                        }
                        match ty {
                            "div" => a.wrapping_div(b),
                            "mod" => a.wrapping_rem(b),
                            "floor_mod" => {
                                a.rem_euclid(b) + if b < 0 && a.rem_euclid(b) != 0 { b } else { 0 }
                            }
                            _ => {
                                if a == i32::MIN && b == -1 {
                                    return Err(overflow());
                                }
                                a.div_euclid(b) - if b < 0 && a.rem_euclid(b) != 0 { 1 } else { 0 }
                            }
                        }
                    }
                    "pow" => {
                        let (base, exp) = (field(self, "base")?, field(self, "exponent")?);
                        if base == 0 && exp == 0 {
                            return Err("Result of 0 to the power of 0 is undefined".into());
                        }
                        if exp < 0 {
                            return Err("Negative exponent".into());
                        }
                        base.checked_pow(exp as u32).ok_or_else(overflow)?
                    }
                    // Math.absExact's message.
                    "abs" => field(self, "input")?.checked_abs().ok_or_else(|| {
                        "Overflow to represent absolute value of Integer.MIN_VALUE".to_owned()
                    })?,
                    "negate" => field(self, "input")?.checked_neg().ok_or_else(overflow)?,
                    "from_float" => {
                        let f = match o.get("input") {
                            Some(x) => self.float(x, d)?,
                            None => 0.0,
                        };
                        if !f.is_finite() {
                            return Err(format!(
                                "Value {} can't be safely converted to int",
                                float_to_string(f)
                            ));
                        }
                        long_to_int(f as i64)?
                    }
                    "uniform" => {
                        let (lo, hi) = (field(self, "min")?, field(self, "max")?);
                        if lo >= hi {
                            lo
                        } else {
                            // Mth.nextInt computes the inclusive bound as a Java int.
                            let bound = hi.wrapping_sub(lo).wrapping_add(1);
                            self.random
                                .next_int_bound(bound)
                                .map_err(|err| err.to_string())?
                                + lo
                        }
                    }
                    "binomial" => {
                        let n = field(self, "n")?;
                        let p = match o.get("p") {
                            Some(x) => self.float(x, d)?,
                            None => 0.0,
                        };
                        (0..n.max(0))
                            .filter(|_| self.random.next_float() < p)
                            .count() as i32
                    }
                    "conditional" | "number_dispatcher" | "weighted_list" => match self.select(o) {
                        Some(x) => self.int(x, d)?,
                        None => 0,
                    },
                    _ => 0,
                })
            }
            _ => Ok(0),
        }
    }
    /// `ContextFloatProvider.getFloatUnsafe`.
    pub fn float(&mut self, v: &Value, depth: u32) -> Result<f32, Arith> {
        if depth > 64 {
            return Ok(0.0);
        }
        let d = depth + 1;
        match v {
            Value::Number(n) => Ok(n.as_f64().unwrap_or(0.0) as f32),
            Value::String(id) => match lookup(&self.providers.floats, id) {
                Some(def) => self.float(def, d),
                None => Ok(0.0),
            },
            Value::Object(o) => {
                let ty = o
                    .get("type")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .trim_start_matches("minecraft:");
                let inputs = |g: &mut Self| -> Result<Vec<f32>, Arith> {
                    let list = match o.get("inputs") {
                        Some(Value::Array(list)) => list.as_slice(),
                        Some(other) => std::slice::from_ref(other),
                        None => &[],
                    };
                    list.iter().map(|x| g.float(x, d)).collect()
                };
                let field = |g: &mut Self, k: &str| -> Result<f32, Arith> {
                    match o.get(k) {
                        Some(x) => g.float(x, d),
                        None => Ok(0.0),
                    }
                };
                Ok(match ty {
                    "constant" => field(self, "value")?,
                    "add" => inputs(self)?.iter().fold(0.0f32, |a, &x| a + x),
                    "mul" => inputs(self)?.iter().fold(1.0f32, |a, &x| a * x),
                    "avg" => {
                        let xs = inputs(self)?;
                        xs.iter().fold(0.0f32, |a, &x| a + x) / xs.len() as f32
                    }
                    "length" => inputs(self)?.iter().fold(0.0f32, |a, &x| a + x * x).sqrt(),
                    "max" => inputs(self)?.into_iter().fold(-f32::MAX, f32::max),
                    "min" => inputs(self)?.into_iter().fold(f32::MAX, f32::min),
                    "sub" => field(self, "left")? - field(self, "right")?,
                    "div" => field(self, "left")? / field(self, "right")?,
                    "mod" => {
                        let right = field(self, "right")?;
                        if right == 0.0 {
                            f32::NAN
                        } else {
                            field(self, "left")? % right
                        }
                    }
                    "pow" => {
                        let (base, exp) = (field(self, "base")?, field(self, "exponent")?);
                        if base == 0.0 && exp == 0.0 {
                            f32::NAN
                        } else {
                            sunrise_core::jmath::pow(f64::from(base), f64::from(exp)) as f32
                        }
                    }
                    "abs" => field(self, "input")?.abs(),
                    "negate" => -field(self, "input")?,
                    "floor" => field(self, "input")?.floor(),
                    // Mth.ceil returns an int.
                    "ceil" => field(self, "input")?.ceil() as i32 as f32,
                    "round" => sunrise_core::jmath::round_f32(field(self, "input")?) as f32,
                    "truncate" => field(self, "input")?.trunc(),
                    "sqrt" => field(self, "input")?.sqrt(),
                    "sin" => sunrise_core::geometry::trig::sin(f64::from(field(self, "input")?)),
                    "cos" => sunrise_core::geometry::trig::cos(f64::from(field(self, "input")?)),
                    "from_int" => match o.get("input") {
                        Some(x) => self.int(x, d)? as f32,
                        None => 0.0,
                    },
                    "uniform" => {
                        let (lo, hi) = (field(self, "min")?, field(self, "max")?);
                        if lo >= hi {
                            lo
                        } else {
                            self.random.next_float() * (hi - lo) + lo
                        }
                    }
                    "conditional" | "number_dispatcher" | "weighted_list" => match self.select(o) {
                        Some(x) => self.float(x, d)?,
                        None => 0.0,
                    },
                    _ => 0.0,
                })
            }
            _ => Ok(0.0),
        }
    }
    /// The branch a conditional, dispatcher or weighted list provider picks.
    fn select<'v>(&mut self, o: &'v serde_json::Map<String, Value>) -> Option<&'v Value> {
        match o
            .get("type")
            .and_then(Value::as_str)
            .map(|ty| ty.trim_start_matches("minecraft:"))
        {
            Some("conditional") => {
                let holds = o
                    .get("condition")
                    .is_some_and(|x| (self.condition)(x, self.random));
                o.get(if holds { "on_true" } else { "on_false" })
            }
            Some("number_dispatcher") => {
                for case in o
                    .get("cases")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                {
                    if case
                        .get("condition")
                        .is_some_and(|x| (self.condition)(x, self.random))
                    {
                        return case.get("value");
                    }
                }
                o.get("default")
            }
            _ => {
                let list = o.get("distribution")?.as_array()?;
                let weight =
                    |entry: &Value| entry.get("weight").and_then(Value::as_i64).unwrap_or(1);
                // WeightedRandom.getTotalWeight sums into a long and requires an int total.
                let total = list.iter().try_fold(0_i32, |total, entry| {
                    let weight = i32::try_from(weight(entry)).ok()?;
                    (weight >= 0).then_some(())?;
                    total.checked_add(weight)
                })?;
                let mut pick = i64::from(self.random.next_int_bound(total).ok()?);
                for entry in list {
                    pick -= weight(entry);
                    if pick < 0 {
                        return entry.get("data");
                    }
                }
                None
            }
        }
    }
}
