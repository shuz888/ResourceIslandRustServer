use std::collections::HashMap;
use std::fs::File;
use serde::{Deserialize, Serialize};
use tracing::trace;
use crate::enums::Items;

pub async fn load_configuration(file_name: &str) -> Result<GameCfg, anyhow::Error>{
    trace!("YAML configurations loading...");
    if std::path::Path::new(file_name).exists() == false{
        let file = File::create(file_name)?;
        serde_yaml::to_writer(file, &GameCfg::with_defaults())?;
    }
    let file = File::open(file_name)?;
    let cfg: GameCfg = serde_yaml::from_reader(file)?;
    Ok(cfg)
}
pub async fn save_configuration(file_name: &str, cfg: GameCfg) -> Result<(), anyhow::Error>{
    let file = File::create(file_name)?;
    serde_yaml::to_writer(file, &cfg)?;
    Ok(())
}
#[derive(Serialize, Deserialize, Debug)]
pub struct GameCfg {
    pub server: ServerCfg,
    pub game_rules: GameRules,
}
impl GameCfg {
    pub fn with_defaults() -> GameCfg {
        GameCfg {
            server: ServerCfg::with_defaults(),
            game_rules: GameRules::with_defaults(),
        }
    }
}
#[derive(Serialize, Deserialize, Debug)]
pub struct ServerCfg {
    pub player_numbers: u32,
    pub use_token: bool,
    pub token: String,
    pub query_use_token: bool,
    pub bind_host: String,
    pub bind_port: u32,
}
impl ServerCfg {
    pub fn with_defaults() -> ServerCfg{
        ServerCfg {
            player_numbers: 4,
            use_token: false,
            token: "set_the_token_here".into(),
            query_use_token: false,
            bind_host: "0.0.0.0".into(),
            bind_port: 8080,
        }
    }
}
#[derive(Serialize, Deserialize, Debug)]
pub struct GameRules {
    pub prepare: PrepareCfg,
    pub resource_values_default: ResourceValuesDefault,
    pub investment: InvestmentCfg,
}
impl GameRules {
    pub fn with_defaults() -> GameRules {
        GameRules {
            prepare: PrepareCfg::with_defaults(),
            resource_values_default: ResourceValuesDefault::with_defaults(),
            investment: InvestmentCfg::with_defaults()
        }
    }
}
#[derive(Serialize, Deserialize, Debug)]
pub struct PrepareCfg {
    pub total_epochs: u32,
    pub draw_cards: u32,
    pub default_ap: u32,
    pub deck: DeckCfg
}
impl PrepareCfg {
    pub fn with_defaults() -> PrepareCfg {
        PrepareCfg {
            total_epochs: 10,
            draw_cards: 10,
            default_ap: 5,
            deck: DeckCfg::with_defaults(),
        }
    }
}
#[derive(Serialize, Deserialize, Debug)]
pub struct DeckCfg {
    pub diamond: u32,
    pub gold: u32,
    pub wood: u32,
    pub ore: u32,
    pub food: u32,
    pub iron: u32,
}
impl DeckCfg {
    pub fn with_defaults() -> DeckCfg {
        DeckCfg {
            diamond: 50,
            gold: 80,
            wood: 100,
            ore: 100,
            food: 100,
            iron: 100,
        }
    }
}
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ResourceValuesDefault {
    pub diamond: u32,
    pub gold: u32,
    pub wood: u32,
    pub ore: u32,
    pub food: u32,
    pub iron: u32,
}
impl ResourceValuesDefault {
    pub fn with_defaults() -> ResourceValuesDefault {
        ResourceValuesDefault {
            diamond: 8,
            gold: 6,
            wood: 2,
            ore: 3,
            food: 1,
            iron: 2,
        }
    }
}
impl Into<HashMap<Items, u32>> for &ResourceValuesDefault {
    fn into(self) -> HashMap<Items, u32> {
        let mut res = HashMap::new();
        res.insert(Items::Diamond, self.diamond);
        res.insert(Items::Gold, self.gold);
        res.insert(Items::Wood, self.wood);
        res.insert(Items::Ore, self.ore);
        res.insert(Items::Food, self.food);
        res.insert(Items::Iron, self.iron);
        res
    }
}
#[derive(Serialize, Deserialize, Debug)]
pub struct InvestmentCfg {
    pub enable: bool,
    pub needs_ap: InvestmentApCosts,
}
impl InvestmentCfg {
    pub fn with_defaults() -> InvestmentCfg {
        InvestmentCfg {
            enable: true,
            needs_ap: InvestmentApCosts::with_defaults(),
        }
    }
}
#[derive(Serialize, Deserialize, Debug)]
pub struct InvestmentApCosts {
    pub explore: u32,
    pub exchange: u32,
    pub build: u32,
    pub open: u32,
    pub bank: u32,
    pub mine: u32,
    pub pick: u32,
}
impl InvestmentApCosts {
    pub fn with_defaults() -> InvestmentApCosts {
        InvestmentApCosts {
            explore: 1,
            exchange: 2,
            build: 3,
            open: 1,
            bank: 0,
            mine: 0,
            pick: 0,
        }
    }
}