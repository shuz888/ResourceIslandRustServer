pub mod config;
pub mod enums;
pub mod game;
pub mod structs;
use thiserror::Error;
#[derive(Error, Debug)]
pub enum NoSuchFound {
    #[error("你传入的{0}物品无法找到对应枚举量")]
    NoSuchItems(&'static str),
    #[error("你传入的{0}建筑无法找到对应枚举量")]
    NoSuchBuildings(&'static str),
}
