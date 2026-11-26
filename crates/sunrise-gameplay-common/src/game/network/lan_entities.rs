

#[cfg(test)]
pub(crate) mod tests {

use super::*;

macro_rules! pending {
        ($list:expr) => {{
            let mut owner = $list.remove($list.len() - 1);
            owner.id = 0;
            $list.push(owner);
        }};
    }
}
