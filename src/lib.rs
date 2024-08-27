#![feature(async_closure)]
#![feature(type_alias_impl_trait)]
#![feature(map_try_insert)]
pub mod core;
mod plugins;

#[cfg(test)]
mod tests {
    use super::*;
}
