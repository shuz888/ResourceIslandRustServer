use crate::enums::Items;
use crate::structs::Player;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;

#[derive(Serialize, Deserialize, Debug)]
pub struct GameCfg {
    pub server: ServerCfg,
    pub game_rules: GameRules,
}
impl GameCfg {
    fn with_defaults() -> GameCfg {
        GameCfg {
            server: Default::default(),
            game_rules: Default::default(),
        }
    }
    pub fn load_from(file_name: String) -> anyhow::Result<Self> {
        if std::path::Path::new(&file_name).exists() == false {
            let file = File::create(&file_name)?;
            serde_yaml::to_writer::<File, GameCfg>(file, &Default::default())?;
        }
        let file = File::open(&file_name)?;
        let cfg = serde_yaml::from_reader(file);
        Ok(cfg?)
    }
}
impl Default for GameCfg {
    fn default() -> Self {
        Self::with_defaults()
    }
}
#[derive(Serialize, Deserialize, Debug)]
pub struct ServerCfg {
    pub player_numbers: u32,
    pub use_token: bool,
    pub token: String,
    pub bind_host: String,
    pub bind_port: u32,
    pub seed: String,
    pub heart_beat_interval: u32,
}
impl ServerCfg {
    fn with_defaults() -> ServerCfg {
        ServerCfg {
            player_numbers: 4,
            use_token: false,
            token: "__set_the_token_here__".into(),
            bind_host: "0.0.0.0".into(),
            bind_port: 8080,
            seed: "__set_seed_here__".into(),
            heart_beat_interval: 10,
        }
    }
}
impl Default for ServerCfg {
    fn default() -> Self {
        Self::with_defaults()
    }
}
#[derive(Serialize, Deserialize, Debug)]
pub struct GameRules {
    pub prepare: PrepareCfg,
    pub resource_values_default: ResourceValuesDefault,
    pub investment: InvestmentCfg,
}
impl GameRules {
    fn with_defaults() -> GameRules {
        GameRules {
            prepare: Default::default(),
            resource_values_default: Default::default(),
            investment: Default::default(),
        }
    }
}
impl Default for GameRules {
    fn default() -> Self {
        Self::with_defaults()
    }
}
#[derive(Serialize, Deserialize, Debug)]
pub struct PrepareCfg {
    pub total_epochs: u32,
    pub draw_cards: u32,
    pub defaults_give_player: DefaultsGivePlayerCfg,
    pub deck: DeckCfg,
}
impl PrepareCfg {
    fn with_defaults() -> PrepareCfg {
        PrepareCfg {
            total_epochs: 10,
            draw_cards: 10,
            defaults_give_player: Default::default(),
            deck: Default::default(),
        }
    }
}
impl Default for PrepareCfg {
    fn default() -> Self {
        Self::with_defaults()
    }
}
#[derive(Serialize, Deserialize, Debug)]
pub struct DefaultsGivePlayerCfg {
    pub ap: u32,
    pub diamond: u32,
    pub gold: u32,
    pub wood: u32,
    pub ore: u32,
    pub food: u32,
    pub iron: u32,
}
impl DefaultsGivePlayerCfg {
    fn with_defaults() -> DefaultsGivePlayerCfg {
        DefaultsGivePlayerCfg {
            ap: 5,
            diamond: 0,
            gold: 0,
            wood: 0,
            ore: 0,
            food: 5,
            iron: 0,
        }
    }
    pub fn apply_to_player(&self, player: &mut Player) {
        player.action_points = self.ap;
        player.resources.insert(Items::Gold, self.gold);
        player.resources.insert(Items::Ore, self.ore);
        player.resources.insert(Items::Diamond, self.diamond);
        player.resources.insert(Items::Wood, self.wood);
        player.resources.insert(Items::Iron, self.iron);
        player.resources.insert(Items::Food, self.food);
    }
}
impl Default for DefaultsGivePlayerCfg {
    fn default() -> Self {
        Self::with_defaults()
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
    fn with_defaults() -> DeckCfg {
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
impl Into<HashMap<Items, u32>> for &DeckCfg {
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
impl Default for DeckCfg {
    fn default() -> Self {
        Self::with_defaults()
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
    fn with_defaults() -> ResourceValuesDefault {
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
impl Default for ResourceValuesDefault {
    fn default() -> Self {
        Self::with_defaults()
    }
}
#[derive(Serialize, Deserialize, Debug)]
pub struct InvestmentCfg {
    pub enable: bool,
    pub needs_ap: InvestmentApCosts,
}
impl InvestmentCfg {
    fn with_defaults() -> InvestmentCfg {
        InvestmentCfg {
            enable: true,
            needs_ap: Default::default(),
        }
    }
}
impl Default for InvestmentCfg {
    fn default() -> Self {
        Self::with_defaults()
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
impl Default for InvestmentApCosts {
    fn default() -> Self {
        Self::with_defaults()
    }
}
