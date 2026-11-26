use super::*;

#[cfg(test)]
thread_local! {
    pub(in crate::game) static DISPATCHES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}
