#[cfg(test)]
pub(crate) mod tests {

    use super::*;

    macro_rules! pending {
        ($list:expr) => {{
            let mut owner = $list.pop().unwrap();
            owner.id = 0;
            $list.push(owner);
        }};
    }
}
