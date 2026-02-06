use crate::enums::Items;
use crate::structs::Player;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;

#[macro_export]
macro_rules! impl_default_with {
    ($type_name:ident) => {
        impl Default for $type_name {
            fn default() -> Self {
                $type_name::with_defaults()
            }
        }
    };
}

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
impl_default_with!(GameCfg);
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
impl_default_with!(ServerCfg);
#[derive(Serialize, Deserialize, Debug)]
pub struct GameRules {
    pub prepare: PrepareCfg,
    pub resource_values_default: ResourceValuesDefault,
    pub investment: InvestmentCfg,
    pub bidding: BiddingCfg,
    pub value_changing: ValueChangingCfg,
    pub events: EventsCfg,
}
impl GameRules {
    fn with_defaults() -> GameRules {
        GameRules {
            prepare: Default::default(),
            resource_values_default: Default::default(),
            investment: Default::default(),
            bidding: Default::default(),
            value_changing: Default::default(),
            events: Default::default(),
        }
    }
}
impl_default_with!(GameRules);
#[derive(Serialize, Deserialize, Debug)]
pub struct BiddingCfg {
    pub enable: bool,
    pub broadcast_bid_message: bool,
    pub bid_min: u32,
    pub bid_max: u32,
}
impl BiddingCfg {
    fn with_defaults() -> Self {
        Self {
            enable: true,
            broadcast_bid_message: true,
            bid_min: 1,
            bid_max: 0,
        }
    }
}
impl_default_with!(BiddingCfg);
#[derive(Serialize, Deserialize, Debug)]
pub struct EventsCfg {
    pub enable: bool,
    pub pirate_attack: PirateAttackCfg,
    pub famine: FamineCfg,
    pub crop_bonus: CropBonusCfg,
    pub ap_bonus: ApBonusCfg,
    pub interval: u32,
}
impl EventsCfg {
    fn with_defaults() -> Self {
        Self {
            enable: true,
            pirate_attack: Default::default(),
            famine: Default::default(),
            crop_bonus: Default::default(),
            ap_bonus: Default::default(),
            interval: 5,
        }
    }
}
impl_default_with!(EventsCfg);
#[derive(Serialize, Deserialize, Debug)]
pub struct ApBonusCfg {
    pub enable: bool,
    pub bonus: f32,
}
impl ApBonusCfg {
    fn with_defaults() -> Self {
        Self {
            enable: true,
            bonus: 2.0,
        }
    }
}
impl_default_with!(ApBonusCfg);
#[derive(Serialize, Deserialize, Debug)]
pub struct FamineCfg {
    pub enable: bool,
    pub need_food: u32,
}
impl FamineCfg {
    fn with_defaults() -> Self {
        Self {
            enable: true,
            need_food: 3,
        }
    }
}
impl_default_with!(FamineCfg);
#[derive(Serialize, Deserialize, Debug)]
pub struct CropBonusCfg {
    pub enable: bool,
    pub bonus: f32,
}
impl CropBonusCfg {
    fn with_defaults() -> Self {
        Self {
            enable: true,
            bonus: 1.0,
        }
    }
}
impl_default_with!(CropBonusCfg);
#[derive(Serialize, Deserialize, Debug)]
pub struct PirateAttackCfg {
    pub enable: bool,
    pub need_gold: u32,
}
impl PirateAttackCfg {
    fn with_defaults() -> Self {
        Self {
            enable: true,
            need_gold: 3,
        }
    }
}
impl_default_with!(PirateAttackCfg);
#[derive(Serialize, Deserialize, Debug)]
pub struct ValueChangingCfg {
    pub enable: bool,
    pub mark_up_when: u32,
    pub discount_when: u32,
    pub mark_up: u32,
    pub discount: u32,
}
impl ValueChangingCfg {
    fn with_defaults() -> Self {
        Self {
            enable: true,
            mark_up_when: 3,
            discount_when: 3,
            mark_up: 1,
            discount: 1,
        }
    }
}
impl_default_with!(ValueChangingCfg);
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
impl_default_with!(PrepareCfg);
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
impl_default_with!(DefaultsGivePlayerCfg);
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
impl_default_with!(DeckCfg);
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
impl_default_with!(ResourceValuesDefault);
#[derive(Serialize, Deserialize, Debug)]
pub struct InvestmentCfg {
    pub enable: bool,
    pub explore: ExploreCfg,
    pub exchange: ExchangeCfg,
    pub build: BuildCfg,
    pub crush: CrushOreCfg,
    pub store: StoreMoneyCfg,
}
impl InvestmentCfg {
    fn with_defaults() -> InvestmentCfg {
        InvestmentCfg {
            enable: true,
            explore: Default::default(),
            exchange: Default::default(),
            build: Default::default(),
            crush: Default::default(),
            store: Default::default(),
        }
    }
}
impl_default_with!(InvestmentCfg);
#[derive(Serialize, Deserialize, Debug)]
pub struct BuildCfg {
    pub enable: bool,
    pub needs_ap: u32,
    pub building_cfg: BuildingCfg,
    pub build_limits: u32,
}
impl BuildCfg {
    fn with_defaults() -> Self {
        Self {
            enable: true,
            needs_ap: 3,
            building_cfg: Default::default(),
            build_limits: 0,
        }
    }
}
impl_default_with!(BuildCfg);
#[derive(Serialize, Deserialize, Debug)]
pub struct StoreMoneyCfg {
    pub enable: bool,
    pub needs_ap: u32,
    pub limits: u32,
}
impl StoreMoneyCfg {
    fn with_defaults() -> Self {
        Self {
            enable: true,
            needs_ap: 3,
            limits: 0,
        }
    }
}
impl_default_with!(StoreMoneyCfg);
#[derive(Serialize, Deserialize, Debug)]
pub struct CrushOreCfg {
    pub enable: bool,
    pub needs_ap: u32,
    pub get_ores: u32,
    pub probabilities: HashMap<Items, f32>,
    pub crush_limits: u32,
}
impl CrushOreCfg {
    fn with_defaults() -> Self {
        let mut prob = HashMap::new();
        prob.insert(Items::Diamond, 0.1);
        prob.insert(Items::Gold, 0.3);
        prob.insert(Items::Iron, 0.4);
        prob.insert(Items::Ore, 0.0);
        prob.insert(Items::Wood, 0.0);
        prob.insert(Items::Food, 0.0);
        Self {
            enable: true,
            needs_ap: 1,
            get_ores: 1,
            probabilities: prob,
            crush_limits: 0,
        }
    }
}
impl_default_with!(CrushOreCfg);
#[derive(Serialize, Deserialize, Debug)]
pub struct BuildingCfg {
    pub resource_points_per_building: u32,
    pub farm: FarmCfg,
    pub super_farm: SuperFarmCfg,
    pub miner: MinerCfg,
    pub super_miner: SuperMinerCfg,
    pub bank: BankCfg,
    pub cannon: CannonCfg,
    pub pickaxe: PickaxeCfg,
    pub lumber: LumberCfg,
}
impl BuildingCfg {
    fn with_defaults() -> Self {
        Self {
            resource_points_per_building: 5,
            farm: Default::default(),
            super_farm: Default::default(),
            miner: Default::default(),
            super_miner: Default::default(),
            bank: Default::default(),
            cannon: Default::default(),
            pickaxe: Default::default(),
            lumber: Default::default(),
        }
    }
}
impl_default_with!(BuildingCfg);
#[derive(Serialize, Deserialize, Debug)]
pub struct ExploreCfg {
    pub enable: bool,
    pub items_per_ap: u32,
    pub explore_limits: u32,
}
impl ExploreCfg {
    fn with_defaults() -> Self {
        Self {
            enable: true,
            items_per_ap: 2,
            explore_limits: 0,
        }
    }
}
impl_default_with!(ExploreCfg);
#[derive(Serialize, Deserialize, Debug)]
pub struct ExchangeCfg {
    pub enable: bool,
    pub ap_per_food: u32,
    pub exchange_limits: u32,
}
impl ExchangeCfg {
    fn with_defaults() -> Self {
        Self {
            enable: true,
            ap_per_food: 2,
            exchange_limits: 0,
        }
    }
}
impl_default_with!(ExchangeCfg);
#[derive(Serialize, Deserialize, Debug)]
pub struct FarmCfg {
    pub enable: bool,
    pub auto_give_food: u32,
    pub recipe: Vec<String>,
}
impl FarmCfg {
    fn with_defaults() -> Self {
        let recipe_vec = vec!["item: Iron*3".to_string(), "item: Wood*2".to_string()];
        Self {
            enable: true,
            auto_give_food: 3,
            recipe: recipe_vec,
        }
    }
}
impl_default_with!(FarmCfg);
#[derive(Serialize, Deserialize, Debug)]
pub struct LumberCfg {
    pub enable: bool,
    pub auto_give_wood: u32,
    pub recipe: Vec<String>,
}
impl LumberCfg {
    fn with_defaults() -> Self {
        let recipe_vec = vec!["item: Iron*3".to_string()];
        Self {
            enable: true,
            auto_give_wood: 3,
            recipe: recipe_vec,
        }
    }
}
impl_default_with!(LumberCfg);
#[derive(Serialize, Deserialize, Debug)]
pub struct SuperFarmCfg {
    pub enable: bool,
    pub auto_give_food: u32,
    pub recipe: Vec<String>,
}
impl SuperFarmCfg {
    fn with_defaults() -> Self {
        let recipe_vec = vec![
            "building: Farm*1".to_string(),
            "item: Wood*2".to_string(),
            "item: Gold*1".to_string(),
        ];
        Self {
            enable: true,
            auto_give_food: 5,
            recipe: recipe_vec,
        }
    }
}
impl_default_with!(SuperFarmCfg);
#[derive(Serialize, Deserialize, Debug)]
pub struct MinerCfg {
    pub enable: bool,
    pub auto_give_ore: u32,
    pub recipe: Vec<String>,
}
impl MinerCfg {
    fn with_defaults() -> Self {
        let recipe_vec = vec!["item: iron*2".to_string(), "item: wood*1".to_string()];
        Self {
            enable: true,
            auto_give_ore: 3,
            recipe: recipe_vec,
        }
    }
}
impl_default_with!(MinerCfg);
#[derive(Serialize, Deserialize, Debug)]
pub struct SuperMinerCfg {
    pub enable: bool,
    pub auto_give_ore: u32,
    pub recipe: Vec<String>,
}
impl SuperMinerCfg {
    fn with_defaults() -> Self {
        let recipe_vec = vec!["item: gold*3".to_string(), "building: miner*1".to_string()];
        Self {
            enable: true,
            auto_give_ore: 5,
            recipe: recipe_vec,
        }
    }
}
impl_default_with!(SuperMinerCfg);
#[derive(Serialize, Deserialize, Debug)]
pub struct BankCfg {
    pub enable: bool,
    pub max_money: u32,
    pub rate: f32,
    pub recipe: Vec<String>,
}
impl BankCfg {
    fn with_defaults() -> Self {
        let recipe_vec = vec!["item: diamond*1".to_string(), "item: gold*3".to_string()];
        Self {
            enable: true,
            max_money: 0,
            rate: 0.5,
            recipe: recipe_vec,
        }
    }
}
impl_default_with!(BankCfg);
#[derive(Serialize, Deserialize, Debug)]
pub struct CannonCfg {
    pub enable: bool,
    pub recipe: Vec<String>,
}
impl CannonCfg {
    fn with_defaults() -> Self {
        let recipe_vec = vec!["item: gold*5".to_string(), "item: iron*1".to_string()];
        Self {
            enable: true,
            recipe: recipe_vec,
        }
    }
}
impl_default_with!(CannonCfg);
#[derive(Serialize, Deserialize, Debug)]
pub struct PickaxeCfg {
    pub enable: bool,
    pub recipe: Vec<String>,
}
impl PickaxeCfg {
    fn with_defaults() -> Self {
        let recipe_vec = vec!["item: iron*5".to_string(), "item: gold*1".to_string()];
        Self {
            enable: true,
            recipe: recipe_vec,
        }
    }
}
impl_default_with!(PickaxeCfg);
