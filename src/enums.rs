use std::convert::TryFrom;
use crate::NoSuchFound;

#[derive(Eq, Hash, PartialEq, Copy, Clone)]
pub enum Items {
    Gold,
    Wood,
    Diamond,
    Ore,
    Food,
    Iron
}
impl TryFrom<&'static str> for Items {
    type Error = NoSuchFound;

    fn try_from(value: &'static str) -> Result<Self, Self::Error> {
        if value == "金币" {
            Ok(Self::Gold)
        }else if value == "钻石" {
            Ok(Self::Diamond)
        }else if value == "木材" {
            Ok(Self::Wood)
        }else if value == "矿石" {
            Ok(Self::Ore)
        }else if value == "铁" {
            Ok(Self::Iron)
        }else if value == "食物" {
            Ok(Self::Food)
        }else {
            Err(NoSuchFound::NoSuchItems(value))
        }
    }
}
impl Into<&'static str> for &Items {
    fn into(self) -> &'static str {
        match self {
            Items::Gold => "金币",
            Items::Wood => "木材",
            Items::Diamond => "钻石",
            Items::Ore => "矿石",
            Items::Food => "食物",
            Items::Iron => "铁"
        }
    }
}
#[derive(Eq, PartialEq, Hash, Copy, Clone)]
pub enum Buildings {
    Farm,
    SuperFarm,
    Miner,
    SuperMiner,
    Bank,
    Cannon
}
impl TryFrom<&'static str> for Buildings {
    type Error = NoSuchFound;

    fn try_from(value: &'static str) -> Result<Self, Self::Error> {
        if value == "农场" {
            Ok(Self::Farm)
        }else if value.contains("农场") && value != "农场" {
            Ok(Self::SuperFarm)
        }else if value == "矿机" {
            Ok(Self::Miner)
        }else if value.contains("矿机") && value != "矿机" {
            Ok(Self::SuperMiner)
        }else if value == "银行" {
            Ok(Self::Bank)
        }else if value == "炮台" {
            Ok(Self::Cannon)
        }else {
            Err(NoSuchFound::NoSuchBuildings(value))
        }
    }
}
impl Into<&'static str> for &Buildings {
    fn into(self) -> &'static str {
        match self {
            Buildings::Farm => "农场",
            Buildings::SuperFarm => "无敌农场",
            Buildings::Miner => "矿机",
            Buildings::SuperMiner => "高级矿机",
            Buildings::Bank => "银行",
            Buildings::Cannon => "炮台",
        }
    }
}
pub enum PlayerToServerMessage {
    
}