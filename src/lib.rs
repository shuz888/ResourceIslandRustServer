use crate::enums::{Building, Items};
use crate::structs::Player;
use std::collections::HashMap;

pub fn parse_recipe(recipe: Vec<&'static str>) -> (HashMap<Items, u32>, HashMap<Building, u32>) {
    let mut items: HashMap<Items, u32> = HashMap::new();
    let mut buildings: HashMap<Building, u32> = HashMap::new();
    for item in recipe {
        let parts: Vec<&str> = item.split(':').collect();
        let prefix = parts[0].trim().to_lowercase(); // "items"
        let rest = parts[1].trim(); // "123*5"
        let numbers: Vec<&str> = rest.split('*').collect();
        let num1 = numbers[0].trim();
        let num2 = numbers[1].trim().parse::<u32>().unwrap();
        if prefix == "item" || prefix == "items" {
            let this_item: Items = Items::try_from(num1).unwrap();
            let amount = items.get(&this_item);
            match amount {
                None => {
                    items.insert(this_item, num2);
                }
                Some(obj) => {
                    items.insert(this_item, obj + num2);
                }
            }
        } else if prefix == "building" || prefix == "buildings" {
            let this_building: Building = Building::try_from(num1).unwrap();
            let amount = buildings.get(&this_building).unwrap();
            let amount = amount + num2;
            buildings.insert(this_building, amount);
        }
    }
    (items, buildings)
}
pub fn count_equal_slice<T: PartialEq>(slice: &[T], target: &T) -> usize {
    slice.iter().filter(|&x| x == target).count()
}
pub fn verify_recipe(
    items: &HashMap<Items, u32>,
    buildings: &HashMap<Building, u32>,
    player: &Player,
) -> bool {
    for item in items.clone() {
        if player.resources.get(&item.0).unwrap() < &item.1 {
            return false;
        }
    }
    let player_buildings = player.buildings.clone();
    for building in buildings.clone() {
        let count = count_equal_slice(&player_buildings, &building.0);
        if count < building.1 as usize {
            return false;
        }
    }
    true
}
pub fn discount_recipe(
    items: &HashMap<Items, u32>,
    buildings: &HashMap<Building, u32>,
    player: &mut Player,
) -> Result<(), ()> {
    for item in items.clone() {
        if player.resources.get(&item.0).unwrap() < &item.1 {
            return Err(());
        } else {
            let borrow = player.resources.get_mut(&item.0).unwrap();
            *borrow -= item.1;
        }
    }
    for building in buildings.clone() {
        let now = count_equal_slice(&player.buildings, &building.0);
        if now < building.1 as usize {
            return Err(());
        } else {
            let mut jian = building.1.clone();
            while jian > 0 {
                jian -= 1;
                let index = player
                    .buildings
                    .iter()
                    .position(|x| *x == building.0)
                    .unwrap();
                player.buildings.remove(index);
            }
        }
    }
    Ok(())
}
pub mod game {
    use crate::enums::{
        BiddingError, Building, ContendingAction, ContendingError, Events, InvestmentAction,
        InvestmentError, Items, PlayerToServerMessage, ServerBroadcastMessage,
        ServerToPlayerMessage,
    };
    use crate::structs::{AppState, GameStateResponse};
    use crate::{discount_recipe, parse_recipe, verify_recipe};
    use rand::Rng;
    use std::collections::{HashMap, HashSet};
    use std::process::exit;
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::sync::mpsc::error::TryRecvError;
    use tokio::time::sleep;
    use tracing::info;
    use uuid::Uuid;

    pub async fn game_main_loop(app_state: Arc<AppState>) {
        let (total_epochs, required_players) = {
            let config = app_state.cfg.clone();
            (
                config.game_rules.prepare.total_epochs,
                config.server.player_numbers,
            )
        };
        loop {
            let game_state = app_state.game_state.read().await;
            if game_state.players.len() as u32 >= required_players {
                break;
            }
        }
        info!("游戏现在开始");
        {
            let mut game_state = app_state.game_state.write().await;
            game_state.started = true;
            let game_state = game_state.downgrade();
            game_state
                .broadcast(ServerBroadcastMessage::GameStart {})
                .await;
        }
        let app_state_heartbeat = app_state.clone();
        tokio::spawn(async move {
            let heartbeat_interval = app_state_heartbeat.cfg.server.heart_beat_interval;
            let dur = Duration::from_secs(heartbeat_interval as u64);
            loop {
                {
                    let state = app_state_heartbeat.game_state.read().await;
                    let resp = GameStateResponse::from(&*state);
                    state
                        .broadcast(ServerBroadcastMessage::HeartBeat {
                            state: resp,
                            interval: heartbeat_interval,
                        })
                        .await;
                }
                sleep(dur).await;
            }
        });
        let mut player_bidding = HashMap::new();
        loop {
            let state = app_state.game_state.read().await;
            if state.epoch > total_epochs {
                let mut player_total_values: HashMap<Uuid, u32> = HashMap::new();
                for (uuid, pl) in state.players.iter() {
                    let mut total_values = 0u32;
                    total_values += (pl.bank_money as f32
                        * (1f32
                            + app_state
                                .cfg
                                .game_rules
                                .investment
                                .build
                                .building_cfg
                                .bank
                                .rate))
                        .floor() as u32;
                    for (item, count) in pl.resources.iter() {
                        let now_value = state.resource_values.get(item).unwrap();
                        total_values += count * now_value;
                    }
                    total_values += pl.buildings.len() as u32;
                    player_total_values.insert(*uuid, total_values);
                }
                let mut vec: Vec<(&Uuid, &u32)> = player_total_values.iter().collect();
                vec.sort_by(|x, y| y.1.cmp(x.1));
                player_total_values = vec.into_iter().map(|(x, y)| (*x, *y)).collect();
                state
                    .broadcast(ServerBroadcastMessage::GameOver {
                        player_total_value: player_total_values,
                    })
                    .await;
                info!("游戏现在结束，感谢您的游玩。");
                exit(0);
            }
            let (cur_phase, cur_epoch) = (state.phase, state.epoch);
            drop(state);
            let mut state = app_state.game_state.write().await;
            let items_per_ap = app_state.cfg.game_rules.investment.explore.items_per_ap;
            let mut draw_cards = 0u32;
            if state.market.is_empty() {
                for pl in state.players.values_mut() {
                    if pl.action_points > 1 {
                        pl.action_points = pl.action_points - 1;
                        draw_cards += items_per_ap;
                    }
                }
            }
            let mut cards: Vec<_> = state.current_deck.drain(..draw_cards as usize).collect();
            state.market.append(&mut cards);
            drop(cards);
            drop(state);
            if cur_phase == 1 {
                if !app_state.cfg.game_rules.investment.enable {
                    let mut game_state = app_state.game_state.write().await;
                    game_state.increase_phase().await;
                    continue;
                }
                let game_state = app_state.game_state.read().await;
                game_state
                    .broadcast(ServerBroadcastMessage::PhaseChanged {
                        phase: cur_phase,
                        epoch: cur_epoch,
                    })
                    .await;
                drop(game_state);
                let mut game_state = app_state.game_state.write().await;
                let mut current_deck = game_state.current_deck.clone();
                for pl in game_state.players.values_mut() {
                    for building in pl.buildings.iter() {
                        match building {
                            Building::Farm => {
                                if app_state
                                    .cfg
                                    .game_rules
                                    .investment
                                    .build
                                    .building_cfg
                                    .farm
                                    .enable
                                {
                                    continue;
                                }
                                let pl_res_food_mut = pl.resources.get_mut(&Items::Food).unwrap();
                                *pl_res_food_mut += app_state
                                    .cfg
                                    .game_rules
                                    .investment
                                    .build
                                    .building_cfg
                                    .farm
                                    .auto_give_food;
                                pl.to_channel
                                    .sender
                                    .clone()
                                    .send(ServerToPlayerMessage::BuildingWorked {
                                        building: Building::Farm,
                                    })
                                    .await
                                    .unwrap();
                            }
                            Building::SuperFarm => {
                                if app_state
                                    .cfg
                                    .game_rules
                                    .investment
                                    .build
                                    .building_cfg
                                    .super_farm
                                    .enable
                                {
                                    continue;
                                }
                                let pl_res_food_mut = pl.resources.get_mut(&Items::Food).unwrap();
                                *pl_res_food_mut += app_state
                                    .cfg
                                    .game_rules
                                    .investment
                                    .build
                                    .building_cfg
                                    .super_farm
                                    .auto_give_food;
                                pl.to_channel
                                    .sender
                                    .clone()
                                    .send(ServerToPlayerMessage::BuildingWorked {
                                        building: Building::SuperFarm,
                                    })
                                    .await
                                    .unwrap();
                            }
                            Building::Miner => {
                                if app_state
                                    .cfg
                                    .game_rules
                                    .investment
                                    .build
                                    .building_cfg
                                    .miner
                                    .enable
                                {
                                    continue;
                                }
                                let mut give_ore = app_state
                                    .cfg
                                    .game_rules
                                    .investment
                                    .build
                                    .building_cfg
                                    .miner
                                    .auto_give_ore;
                                while give_ore > 0 {
                                    give_ore -= 1;
                                    let ore_index = current_deck
                                        .iter()
                                        .position(|x| match x {
                                            Items::Gold => true,
                                            Items::Diamond => true,
                                            Items::Ore => true,
                                            Items::Iron => true,
                                            _ => false,
                                        })
                                        .unwrap();
                                    let ore = current_deck.remove(ore_index);
                                    let now_ore_count = pl.resources.get(&ore).unwrap();
                                    pl.resources.insert(ore, now_ore_count + 1);
                                }
                            }
                            Building::SuperMiner => {
                                if app_state
                                    .cfg
                                    .game_rules
                                    .investment
                                    .build
                                    .building_cfg
                                    .super_miner
                                    .enable
                                {
                                    continue;
                                }
                                let mut give_ore = app_state
                                    .cfg
                                    .game_rules
                                    .investment
                                    .build
                                    .building_cfg
                                    .super_miner
                                    .auto_give_ore;
                                while give_ore > 0 {
                                    give_ore -= 1;
                                    let ore_index = current_deck
                                        .iter()
                                        .position(|x| match x {
                                            Items::Gold => true,
                                            Items::Diamond => true,
                                            Items::Ore => true,
                                            Items::Iron => true,
                                            _ => false,
                                        })
                                        .unwrap();
                                    let ore = current_deck.remove(ore_index);
                                    let now_ore_count = pl.resources.get(&ore).unwrap();
                                    pl.resources.insert(ore, now_ore_count + 1);
                                }
                            }
                            Building::Lumber => {
                                if app_state
                                    .cfg
                                    .game_rules
                                    .investment
                                    .build
                                    .building_cfg
                                    .lumber
                                    .enable
                                {
                                    continue;
                                }
                                let mut give_wood = app_state
                                    .cfg
                                    .game_rules
                                    .investment
                                    .build
                                    .building_cfg
                                    .lumber
                                    .auto_give_wood;
                                while give_wood > 0 {
                                    give_wood -= 1;
                                    let ore_index =
                                        current_deck.iter().position(|x| x == &Items::Wood);
                                    if ore_index == None {
                                        break;
                                    }
                                    let ore = current_deck.remove(ore_index.unwrap());
                                    let now_ore_count = pl.resources.get(&ore).unwrap();
                                    pl.resources.insert(ore, now_ore_count + 1);
                                }
                            }
                            _ => {}
                        }
                    }
                }
                game_state.current_deck = current_deck;
                let game_state = game_state.downgrade();
                for pl in game_state.players.keys() {
                    game_state
                        .send_to(
                            pl,
                            ServerToPlayerMessage::DataRequired {
                                phase: cur_phase,
                                epoch: cur_epoch,
                            },
                        )
                        .await;
                }
                let game_state = app_state.game_state.read().await;
                let mut investment_unfinished: HashSet<Uuid> =
                    game_state.players.keys().map(|pl| pl.clone()).collect();
                let mut counts: HashMap<Uuid, HashMap<&'static str, u32>> = game_state
                    .players
                    .keys()
                    .map(|pl| {
                        let actions: Vec<&'static str> =
                            vec!["exchange", "explore", "build", "crush", "store"];
                        let action_map: HashMap<&'static str, u32> =
                            actions.into_iter().map(|s| (s, 0)).collect();
                        (pl.clone(), action_map)
                    })
                    .collect();
                drop(game_state);
                loop {
                    if investment_unfinished.is_empty() {
                        break;
                    }
                    let senders_and_receivers: Vec<_> = {
                        let game_state = app_state.game_state.read().await;
                        game_state
                            .players
                            .iter()
                            .map(|(uuid, player)| {
                                (
                                    uuid.clone(),
                                    (
                                        player.from_channel.sender.clone(),
                                        player.from_channel.receiver.clone(),
                                    ),
                                )
                            })
                            .collect()
                    };
                    for (uuid, (sender, receiver)) in senders_and_receivers {
                        if !investment_unfinished.contains(&uuid) {
                            continue;
                        }
                        let mut receiver_locked = receiver.lock().await;
                        let msg = receiver_locked.try_recv();
                        drop(receiver_locked);
                        if msg.is_err() {
                            let err = msg.err().unwrap();
                            match err {
                                TryRecvError::Empty => continue,
                                TryRecvError::Disconnected => {
                                    let mut game_state = app_state.game_state.write().await;
                                    game_state.unregister_player(&uuid).await;
                                }
                            }
                            continue;
                        }
                        let msg = msg.unwrap();
                        match msg {
                            PlayerToServerMessage::SendInvestment { action } => match action {
                                InvestmentAction::Explore {} => {
                                    let state = app_state.game_state.read().await;
                                    if !app_state.cfg.game_rules.investment.explore.enable {
                                        state
                                            .send_to(
                                                &uuid,
                                                ServerToPlayerMessage::InvestmentResult {
                                                    action,
                                                    error: true,
                                                    reason: Some(
                                                        InvestmentError::ActionIsNotEnabled {},
                                                    ),
                                                },
                                            )
                                            .await;
                                        continue;
                                    }
                                    let items_per_ap =
                                        app_state.cfg.game_rules.investment.explore.items_per_ap;
                                    let explore_limits =
                                        app_state.cfg.game_rules.investment.explore.explore_limits;
                                    let now_count =
                                        counts.get(&uuid).unwrap().get("explore").unwrap().clone();
                                    if now_count > explore_limits && explore_limits != 0 {
                                        state
                                            .send_to(
                                                &uuid,
                                                ServerToPlayerMessage::InvestmentResult {
                                                    action,
                                                    error: true,
                                                    reason: Some(InvestmentError::LimitsExceeded {
                                                        limit: explore_limits,
                                                    }),
                                                },
                                            )
                                            .await;
                                        continue;
                                    } else {
                                        counts
                                            .get_mut(&uuid)
                                            .unwrap()
                                            .insert("explore", now_count + 1);
                                    }
                                    if state.players.get(&uuid).unwrap().action_points < 1 {
                                        state
                                            .send_to(
                                                &uuid,
                                                ServerToPlayerMessage::InvestmentResult {
                                                    action,
                                                    error: true,
                                                    reason: Some(
                                                        InvestmentError::NoEnoughActionPoints {
                                                            need: 1,
                                                        },
                                                    ),
                                                },
                                            )
                                            .await;
                                        continue;
                                    }
                                    drop(state);
                                    let mut game_state = app_state.game_state.write().await;
                                    game_state.players.get_mut(&uuid).unwrap().action_points -= 1;
                                    let mut cards: Vec<_> = game_state
                                        .current_deck
                                        .drain(..items_per_ap as usize)
                                        .collect();
                                    game_state.market.append(&mut cards);
                                    drop(cards);
                                    let game_state = game_state.downgrade();
                                    game_state
                                        .send_to(
                                            &uuid,
                                            ServerToPlayerMessage::InvestmentResult {
                                                action,
                                                error: false,
                                                reason: None,
                                            },
                                        )
                                        .await;
                                }
                                InvestmentAction::Exchange {} => {
                                    let state = app_state.game_state.read().await;
                                    if !app_state.cfg.game_rules.investment.exchange.enable {
                                        state
                                            .send_to(
                                                &uuid,
                                                ServerToPlayerMessage::InvestmentResult {
                                                    action,
                                                    error: true,
                                                    reason: Some(
                                                        InvestmentError::ActionIsNotEnabled {},
                                                    ),
                                                },
                                            )
                                            .await;
                                        continue;
                                    }
                                    let ap_per_food =
                                        app_state.cfg.game_rules.investment.exchange.ap_per_food;
                                    let exchange_limits = app_state
                                        .cfg
                                        .game_rules
                                        .investment
                                        .exchange
                                        .exchange_limits;
                                    let now_limits =
                                        counts.get(&uuid).unwrap().get("exchange").unwrap().clone();
                                    if now_limits > exchange_limits && exchange_limits != 0 {
                                        let game_state = app_state.game_state.read().await;
                                        game_state
                                            .send_to(
                                                &uuid,
                                                ServerToPlayerMessage::InvestmentResult {
                                                    action,
                                                    error: true,
                                                    reason: Some(InvestmentError::LimitsExceeded {
                                                        limit: exchange_limits,
                                                    }),
                                                },
                                            )
                                            .await;
                                        continue;
                                    }
                                    let mut game_state = app_state.game_state.write().await;
                                    let player_mut = game_state.players.get_mut(&uuid).unwrap();
                                    if player_mut.resources.get(&Items::Food).unwrap() < &1 {
                                        game_state
                                            .send_to(
                                                &uuid,
                                                ServerToPlayerMessage::InvestmentResult {
                                                    action,
                                                    error: true,
                                                    reason: Some(InvestmentError::NoEnoughFood {
                                                        need: 1,
                                                    }),
                                                },
                                            )
                                            .await;
                                        continue;
                                    }
                                    let old =
                                        player_mut.resources.get(&Items::Food).unwrap().clone();
                                    player_mut.resources.insert(Items::Food, old - 1);
                                    player_mut.action_points += ap_per_food;
                                    game_state
                                        .send_to(
                                            &uuid,
                                            ServerToPlayerMessage::InvestmentResult {
                                                action,
                                                error: false,
                                                reason: None,
                                            },
                                        )
                                        .await;
                                }
                                InvestmentAction::Build { building } => {
                                    let limits =
                                        app_state.cfg.game_rules.investment.build.build_limits;
                                    if counts.get(&uuid).unwrap().get("build").unwrap() > &limits
                                        && limits != 0
                                    {
                                        let state = app_state.game_state.read().await;
                                        state
                                            .send_to(
                                                &uuid,
                                                ServerToPlayerMessage::InvestmentResult {
                                                    action,
                                                    error: true,
                                                    reason: Some(InvestmentError::LimitsExceeded {
                                                        limit: limits,
                                                    }),
                                                },
                                            )
                                            .await;
                                        continue;
                                    }
                                    let state = app_state.game_state.read().await;
                                    if !app_state.cfg.game_rules.investment.build.enable {
                                        state
                                            .send_to(
                                                &uuid,
                                                ServerToPlayerMessage::InvestmentResult {
                                                    action,
                                                    error: true,
                                                    reason: Some(
                                                        InvestmentError::ActionIsNotEnabled {},
                                                    ),
                                                },
                                            )
                                            .await;
                                        continue;
                                    }
                                    drop(state);
                                    match building {
                                        Building::Farm => {
                                            let state = app_state.game_state.read().await;
                                            if !app_state
                                                .cfg
                                                .game_rules
                                                .investment
                                                .build
                                                .building_cfg
                                                .farm
                                                .enable
                                            {
                                                state.send_to(&uuid, ServerToPlayerMessage::InvestmentResult {
                                                    action,
                                                    error: true,
                                                    reason: Some(InvestmentError::BuildingIsNotEnabled {}),
                                                }).await;
                                                continue;
                                            }
                                            let recipe = app_state
                                                .cfg
                                                .game_rules
                                                .investment
                                                .build
                                                .building_cfg
                                                .farm
                                                .recipe
                                                .clone();
                                            let recipe = recipe
                                                .into_iter()
                                                .map(|x| x.leak() as &'static str)
                                                .collect::<Vec<&'static str>>();
                                            let recipe = parse_recipe(recipe);
                                            drop(state);
                                            let mut state = app_state.game_state.write().await;
                                            let player = state.players.get_mut(&uuid).unwrap();
                                            if !verify_recipe(&recipe.0, &recipe.1, player) {
                                                state.send_to(&uuid, ServerToPlayerMessage::InvestmentResult {
                                                    action,
                                                    error: true,
                                                    reason: Some(InvestmentError::NoEnoughMaterials {
                                                        need_items: recipe.0.clone(),
                                                        need_buildings: recipe.1.clone(),
                                                    }),
                                                }).await;
                                                continue;
                                            }
                                            discount_recipe(&recipe.0, &recipe.1, player).unwrap();
                                            player.buildings.push(building.clone());
                                            let state = state.downgrade();
                                            state
                                                .send_to(
                                                    &uuid,
                                                    ServerToPlayerMessage::InvestmentResult {
                                                        action,
                                                        error: false,
                                                        reason: None,
                                                    },
                                                )
                                                .await;
                                        }
                                        Building::SuperFarm => {
                                            let state = app_state.game_state.read().await;
                                            if !app_state
                                                .cfg
                                                .game_rules
                                                .investment
                                                .build
                                                .building_cfg
                                                .super_farm
                                                .enable
                                            {
                                                state.send_to(&uuid, ServerToPlayerMessage::InvestmentResult {
                                                    action,
                                                    error: true,
                                                    reason: Some(InvestmentError::BuildingIsNotEnabled {}),
                                                }).await;
                                                continue;
                                            }
                                            let recipe = app_state
                                                .cfg
                                                .game_rules
                                                .investment
                                                .build
                                                .building_cfg
                                                .super_farm
                                                .recipe
                                                .clone();
                                            let recipe = recipe
                                                .into_iter()
                                                .map(|x| x.leak() as &'static str)
                                                .collect::<Vec<&'static str>>();
                                            let recipe = parse_recipe(recipe);
                                            drop(state);
                                            let mut state = app_state.game_state.write().await;
                                            let player = state.players.get_mut(&uuid).unwrap();
                                            if !verify_recipe(&recipe.0, &recipe.1, player) {
                                                state.send_to(&uuid, ServerToPlayerMessage::InvestmentResult {
                                                    action,
                                                    error: true,
                                                    reason: Some(InvestmentError::NoEnoughMaterials {
                                                        need_items: recipe.0.clone(),
                                                        need_buildings: recipe.1.clone(),
                                                    }),
                                                }).await;
                                                continue;
                                            }
                                            discount_recipe(&recipe.0, &recipe.1, player).unwrap();
                                            player.buildings.push(building.clone());
                                            let state = state.downgrade();
                                            state
                                                .send_to(
                                                    &uuid,
                                                    ServerToPlayerMessage::InvestmentResult {
                                                        action,
                                                        error: false,
                                                        reason: None,
                                                    },
                                                )
                                                .await;
                                        }
                                        Building::Miner => {
                                            let state = app_state.game_state.read().await;
                                            if !app_state
                                                .cfg
                                                .game_rules
                                                .investment
                                                .build
                                                .building_cfg
                                                .miner
                                                .enable
                                            {
                                                state.send_to(&uuid, ServerToPlayerMessage::InvestmentResult {
                                                    action,
                                                    error: true,
                                                    reason: Some(InvestmentError::BuildingIsNotEnabled {}),
                                                }).await;
                                                continue;
                                            }
                                            let recipe = app_state
                                                .cfg
                                                .game_rules
                                                .investment
                                                .build
                                                .building_cfg
                                                .miner
                                                .recipe
                                                .clone();
                                            let recipe = recipe
                                                .into_iter()
                                                .map(|x| x.leak() as &'static str)
                                                .collect::<Vec<&'static str>>();
                                            let recipe = parse_recipe(recipe);
                                            drop(state);
                                            let mut state = app_state.game_state.write().await;
                                            let player = state.players.get_mut(&uuid).unwrap();
                                            if !verify_recipe(&recipe.0, &recipe.1, player) {
                                                state.send_to(&uuid, ServerToPlayerMessage::InvestmentResult {
                                                    action,
                                                    error: true,
                                                    reason: Some(InvestmentError::NoEnoughMaterials {
                                                        need_items: recipe.0.clone(),
                                                        need_buildings: recipe.1.clone(),
                                                    }),
                                                }).await;
                                                continue;
                                            }
                                            discount_recipe(&recipe.0, &recipe.1, player).unwrap();
                                            player.buildings.push(building.clone());
                                            let state = state.downgrade();
                                            state
                                                .send_to(
                                                    &uuid,
                                                    ServerToPlayerMessage::InvestmentResult {
                                                        action,
                                                        error: false,
                                                        reason: None,
                                                    },
                                                )
                                                .await;
                                        }
                                        Building::SuperMiner => {
                                            let state = app_state.game_state.read().await;
                                            if !app_state
                                                .cfg
                                                .game_rules
                                                .investment
                                                .build
                                                .building_cfg
                                                .super_miner
                                                .enable
                                            {
                                                state.send_to(&uuid, ServerToPlayerMessage::InvestmentResult {
                                                    action,
                                                    error: true,
                                                    reason: Some(InvestmentError::BuildingIsNotEnabled {}),
                                                }).await;
                                                continue;
                                            }
                                            let recipe = app_state
                                                .cfg
                                                .game_rules
                                                .investment
                                                .build
                                                .building_cfg
                                                .super_miner
                                                .recipe
                                                .clone();
                                            let recipe = recipe
                                                .into_iter()
                                                .map(|x| x.leak() as &'static str)
                                                .collect::<Vec<&'static str>>();
                                            let recipe = parse_recipe(recipe);
                                            drop(state);
                                            let mut state = app_state.game_state.write().await;
                                            let player = state.players.get_mut(&uuid).unwrap();
                                            if !verify_recipe(&recipe.0, &recipe.1, player) {
                                                state.send_to(&uuid, ServerToPlayerMessage::InvestmentResult {
                                                    action,
                                                    error: true,
                                                    reason: Some(InvestmentError::NoEnoughMaterials {
                                                        need_items: recipe.0.clone(),
                                                        need_buildings: recipe.1.clone(),
                                                    }),
                                                }).await;
                                                continue;
                                            }
                                            discount_recipe(&recipe.0, &recipe.1, player).unwrap();
                                            player.buildings.push(building.clone());
                                            let state = state.downgrade();
                                            state
                                                .send_to(
                                                    &uuid,
                                                    ServerToPlayerMessage::InvestmentResult {
                                                        action,
                                                        error: false,
                                                        reason: None,
                                                    },
                                                )
                                                .await;
                                        }
                                        Building::Bank => {
                                            let state = app_state.game_state.read().await;
                                            if !app_state
                                                .cfg
                                                .game_rules
                                                .investment
                                                .build
                                                .building_cfg
                                                .bank
                                                .enable
                                            {
                                                state.send_to(&uuid, ServerToPlayerMessage::InvestmentResult {
                                                    action,
                                                    error: true,
                                                    reason: Some(InvestmentError::BuildingIsNotEnabled {}),
                                                }).await;
                                                continue;
                                            }
                                            let recipe = app_state
                                                .cfg
                                                .game_rules
                                                .investment
                                                .build
                                                .building_cfg
                                                .bank
                                                .recipe
                                                .clone();
                                            let recipe = recipe
                                                .into_iter()
                                                .map(|x| x.leak() as &'static str)
                                                .collect::<Vec<&'static str>>();
                                            let recipe = parse_recipe(recipe);
                                            drop(state);
                                            let mut state = app_state.game_state.write().await;
                                            let player = state.players.get_mut(&uuid).unwrap();
                                            if !verify_recipe(&recipe.0, &recipe.1, player) {
                                                state.send_to(&uuid, ServerToPlayerMessage::InvestmentResult {
                                                    action,
                                                    error: true,
                                                    reason: Some(InvestmentError::NoEnoughMaterials {
                                                        need_items: recipe.0.clone(),
                                                        need_buildings: recipe.1.clone(),
                                                    }),
                                                }).await;
                                                continue;
                                            }
                                            discount_recipe(&recipe.0, &recipe.1, player).unwrap();
                                            player.buildings.push(building.clone());
                                            let state = state.downgrade();
                                            state
                                                .send_to(
                                                    &uuid,
                                                    ServerToPlayerMessage::InvestmentResult {
                                                        action,
                                                        error: false,
                                                        reason: None,
                                                    },
                                                )
                                                .await;
                                        }
                                        Building::Cannon => {
                                            let state = app_state.game_state.read().await;
                                            if !app_state
                                                .cfg
                                                .game_rules
                                                .investment
                                                .build
                                                .building_cfg
                                                .cannon
                                                .enable
                                            {
                                                state.send_to(&uuid, ServerToPlayerMessage::InvestmentResult {
                                                    action,
                                                    error: true,
                                                    reason: Some(InvestmentError::BuildingIsNotEnabled {}),
                                                }).await;
                                                continue;
                                            }
                                            let recipe = app_state
                                                .cfg
                                                .game_rules
                                                .investment
                                                .build
                                                .building_cfg
                                                .cannon
                                                .recipe
                                                .clone();
                                            let recipe = recipe
                                                .into_iter()
                                                .map(|x| x.leak() as &'static str)
                                                .collect::<Vec<&'static str>>();
                                            let recipe = parse_recipe(recipe);
                                            drop(state);
                                            let mut state = app_state.game_state.write().await;
                                            let player = state.players.get_mut(&uuid).unwrap();
                                            if !verify_recipe(&recipe.0, &recipe.1, player) {
                                                state.send_to(&uuid, ServerToPlayerMessage::InvestmentResult {
                                                    action,
                                                    error: true,
                                                    reason: Some(InvestmentError::NoEnoughMaterials {
                                                        need_items: recipe.0.clone(),
                                                        need_buildings: recipe.1.clone(),
                                                    }),
                                                }).await;
                                                continue;
                                            }
                                            discount_recipe(&recipe.0, &recipe.1, player).unwrap();
                                            player.buildings.push(building.clone());
                                            let state = state.downgrade();
                                            state
                                                .send_to(
                                                    &uuid,
                                                    ServerToPlayerMessage::InvestmentResult {
                                                        action,
                                                        error: false,
                                                        reason: None,
                                                    },
                                                )
                                                .await;
                                        }
                                        Building::Pickaxe => {
                                            let state = app_state.game_state.read().await;
                                            if !app_state
                                                .cfg
                                                .game_rules
                                                .investment
                                                .build
                                                .building_cfg
                                                .pickaxe
                                                .enable
                                            {
                                                state.send_to(&uuid, ServerToPlayerMessage::InvestmentResult {
                                                    action,
                                                    error: true,
                                                    reason: Some(InvestmentError::BuildingIsNotEnabled {}),
                                                }).await;
                                                continue;
                                            }
                                            let recipe = app_state
                                                .cfg
                                                .game_rules
                                                .investment
                                                .build
                                                .building_cfg
                                                .pickaxe
                                                .recipe
                                                .clone();
                                            let recipe = recipe
                                                .into_iter()
                                                .map(|x| x.leak() as &'static str)
                                                .collect::<Vec<&'static str>>();
                                            let recipe = parse_recipe(recipe);
                                            drop(state);
                                            let mut state = app_state.game_state.write().await;
                                            let player = state.players.get_mut(&uuid).unwrap();
                                            if !verify_recipe(&recipe.0, &recipe.1, player) {
                                                state.send_to(&uuid, ServerToPlayerMessage::InvestmentResult {
                                                    action,
                                                    error: true,
                                                    reason: Some(InvestmentError::NoEnoughMaterials {
                                                        need_items: recipe.0.clone(),
                                                        need_buildings: recipe.1.clone(),
                                                    }),
                                                }).await;
                                                continue;
                                            }
                                            discount_recipe(&recipe.0, &recipe.1, player).unwrap();
                                            player.buildings.push(building.clone());
                                            let state = state.downgrade();
                                            state
                                                .send_to(
                                                    &uuid,
                                                    ServerToPlayerMessage::InvestmentResult {
                                                        action,
                                                        error: false,
                                                        reason: None,
                                                    },
                                                )
                                                .await;
                                        }
                                        Building::Lumber {} => {
                                            let state = app_state.game_state.read().await;
                                            if !app_state
                                                .cfg
                                                .game_rules
                                                .investment
                                                .build
                                                .building_cfg
                                                .lumber
                                                .enable
                                            {
                                                state.send_to(&uuid, ServerToPlayerMessage::InvestmentResult {
                                                    action,
                                                    error: true,
                                                    reason: Some(InvestmentError::BuildingIsNotEnabled {}),
                                                }).await;
                                                continue;
                                            }
                                            let recipe = app_state
                                                .cfg
                                                .game_rules
                                                .investment
                                                .build
                                                .building_cfg
                                                .lumber
                                                .recipe
                                                .clone();
                                            let recipe = recipe
                                                .into_iter()
                                                .map(|x| x.leak() as &'static str)
                                                .collect::<Vec<&'static str>>();
                                            let recipe = parse_recipe(recipe);
                                            drop(state);
                                            let mut state = app_state.game_state.write().await;
                                            let player = state.players.get_mut(&uuid).unwrap();
                                            if !verify_recipe(&recipe.0, &recipe.1, player) {
                                                state.send_to(&uuid, ServerToPlayerMessage::InvestmentResult {
                                                    action,
                                                    error: true,
                                                    reason: Some(InvestmentError::NoEnoughMaterials {
                                                        need_items: recipe.0.clone(),
                                                        need_buildings: recipe.1.clone(),
                                                    }),
                                                }).await;
                                                continue;
                                            }
                                            discount_recipe(&recipe.0, &recipe.1, player).unwrap();
                                            player.buildings.push(building.clone());
                                            let state = state.downgrade();
                                            state
                                                .send_to(
                                                    &uuid,
                                                    ServerToPlayerMessage::InvestmentResult {
                                                        action,
                                                        error: false,
                                                        reason: None,
                                                    },
                                                )
                                                .await;
                                        }
                                    }
                                }
                                InvestmentAction::CrushOre {} => {
                                    let mut state = app_state.game_state.write().await;
                                    if !app_state.cfg.game_rules.investment.crush.enable {
                                        state
                                            .send_to(
                                                &uuid,
                                                ServerToPlayerMessage::InvestmentResult {
                                                    action,
                                                    error: true,
                                                    reason: Some(
                                                        InvestmentError::ActionIsNotEnabled {},
                                                    ),
                                                },
                                            )
                                            .await;
                                        continue;
                                    }
                                    let limits =
                                        app_state.cfg.game_rules.investment.crush.crush_limits;
                                    if counts.get(&uuid).unwrap().get("crush").unwrap() > &limits {
                                        state
                                            .send_to(
                                                &uuid,
                                                ServerToPlayerMessage::InvestmentResult {
                                                    action,
                                                    error: true,
                                                    reason: Some(InvestmentError::LimitsExceeded {
                                                        limit: limits,
                                                    }),
                                                },
                                            )
                                            .await;
                                        continue;
                                    }
                                    let player = state.players.get_mut(&uuid).unwrap();
                                    let needs_ap =
                                        app_state.cfg.game_rules.investment.crush.needs_ap;
                                    if player.action_points < needs_ap {
                                        state
                                            .send_to(
                                                &uuid,
                                                ServerToPlayerMessage::InvestmentResult {
                                                    action,
                                                    error: true,
                                                    reason: Some(
                                                        InvestmentError::NoEnoughActionPoints {
                                                            need: needs_ap,
                                                        },
                                                    ),
                                                },
                                            )
                                            .await;
                                        continue;
                                    }
                                    player.action_points -= needs_ap;
                                    let ore_count = player.resources.get(&Items::Ore).unwrap();
                                    if ore_count < &1 {
                                        state
                                            .send_to(
                                                &uuid,
                                                ServerToPlayerMessage::InvestmentResult {
                                                    action,
                                                    error: true,
                                                    reason: Some(InvestmentError::NoEnoughOre {
                                                        need: 1,
                                                    }),
                                                },
                                            )
                                            .await;
                                        continue;
                                    }
                                    player.resources.insert(Items::Ore, ore_count - 1);
                                    let items: Vec<Items> = app_state
                                        .cfg
                                        .game_rules
                                        .investment
                                        .crush
                                        .probabilities
                                        .keys()
                                        .cloned()
                                        .collect();
                                    let probs: Vec<f32> = app_state
                                        .cfg
                                        .game_rules
                                        .investment
                                        .crush
                                        .probabilities
                                        .values()
                                        .cloned()
                                        .collect();
                                    let mut count =
                                        app_state.cfg.game_rules.investment.crush.get_ores;
                                    let mut rng = app_state.rng.clone();
                                    while count > 0 {
                                        let random_val: f32 = rng.random();
                                        let mut cumulative = 0.0;
                                        for (i, &prob) in probs.iter().enumerate() {
                                            cumulative += prob;
                                            if random_val <= cumulative {
                                                let drawed_item = items[i];
                                                player.resources.insert(drawed_item, 1);
                                            }
                                        }
                                        count -= 1;
                                    }
                                    state
                                        .send_to(
                                            &uuid,
                                            ServerToPlayerMessage::InvestmentResult {
                                                action,
                                                error: false,
                                                reason: None,
                                            },
                                        )
                                        .await;
                                }
                                InvestmentAction::StoreMoney { item, count } => {
                                    let state = app_state.game_state.read().await;
                                    if !app_state.cfg.game_rules.investment.store.enable {
                                        state
                                            .send_to(
                                                &uuid,
                                                ServerToPlayerMessage::InvestmentResult {
                                                    action,
                                                    error: true,
                                                    reason: Some(
                                                        InvestmentError::ActionIsNotEnabled {},
                                                    ),
                                                },
                                            )
                                            .await;
                                        continue;
                                    }
                                    if !app_state
                                        .cfg
                                        .game_rules
                                        .investment
                                        .build
                                        .building_cfg
                                        .bank
                                        .enable
                                    {
                                        state
                                            .send_to(
                                                &uuid,
                                                ServerToPlayerMessage::InvestmentResult {
                                                    action,
                                                    error: true,
                                                    reason: Some(
                                                        InvestmentError::BuildingIsNotEnabled {},
                                                    ),
                                                },
                                            )
                                            .await;
                                        continue;
                                    }
                                    let limits = app_state.cfg.game_rules.investment.store.limits;
                                    if counts.get(&uuid).unwrap().get("store").unwrap() > &limits
                                        && limits != 0
                                    {
                                        state
                                            .send_to(
                                                &uuid,
                                                ServerToPlayerMessage::InvestmentResult {
                                                    action,
                                                    error: true,
                                                    reason: Some(InvestmentError::LimitsExceeded {
                                                        limit: limits,
                                                    }),
                                                },
                                            )
                                            .await;
                                        continue;
                                    }
                                    let player = state.players.get(&uuid).unwrap();
                                    *(counts.get_mut(&uuid).unwrap().get_mut("store").unwrap()) +=
                                        1;
                                    let mut items = HashMap::new();
                                    let buildings = HashMap::new();
                                    items.insert(item, count);
                                    if !verify_recipe(&items, &buildings, player) {
                                        state
                                            .send_to(
                                                &uuid,
                                                ServerToPlayerMessage::InvestmentResult {
                                                    action,
                                                    error: true,
                                                    reason: Some(InvestmentError::NoEnoughItem {
                                                        need: items,
                                                    }),
                                                },
                                            )
                                            .await;
                                        continue;
                                    }
                                    drop(state);
                                    {
                                        let mut state = app_state.game_state.write().await;
                                        let player = state.players.get_mut(&uuid).unwrap();
                                        discount_recipe(&items, &buildings, player).unwrap();
                                    }
                                    let mut state = app_state.game_state.write().await;
                                    let value = count * state.resource_values.get(&item).unwrap();
                                    let player = state.players.get_mut(&uuid).unwrap();
                                    player.bank_money += value;
                                    state
                                        .send_to(
                                            &uuid,
                                            ServerToPlayerMessage::InvestmentResult {
                                                action,
                                                error: false,
                                                reason: None,
                                            },
                                        )
                                        .await;
                                }
                                InvestmentAction::End {} => {
                                    investment_unfinished.remove(&uuid);
                                    let state = app_state.game_state.read().await;
                                    state
                                        .send_to(
                                            &uuid,
                                            ServerToPlayerMessage::InvestmentResult {
                                                action,
                                                error: false,
                                                reason: None,
                                            },
                                        )
                                        .await;
                                }
                            },
                            _ => {
                                sender.send(msg).await.unwrap();
                            }
                        }
                    }
                }
            } else if cur_phase == 2 {
                if !app_state.cfg.game_rules.bidding.enable {
                    let mut state = app_state.game_state.write().await;
                    state.increase_phase().await;
                    continue;
                }
                let state = app_state.game_state.read().await;
                state
                    .broadcast(ServerBroadcastMessage::PhaseChanged {
                        epoch: cur_epoch,
                        phase: cur_phase,
                    })
                    .await;
                for x in state.players.values() {
                    x.to_channel
                        .sender
                        .send(ServerToPlayerMessage::DataRequired {
                            epoch: cur_epoch,
                            phase: cur_phase,
                        })
                        .await
                        .unwrap();
                }
                let mut bidding_unfinished: HashSet<Uuid> = state.players.keys().cloned().collect();
                drop(state);
                drop(player_bidding);
                player_bidding = HashMap::new();
                loop {
                    if bidding_unfinished.is_empty() {
                        break;
                    }
                    let senders_and_receivers: Vec<_> = {
                        let game_state = app_state.game_state.read().await;
                        game_state
                            .players
                            .iter()
                            .map(|(uuid, player)| {
                                (
                                    uuid.clone(),
                                    (
                                        player.from_channel.sender.clone(),
                                        player.from_channel.receiver.clone(),
                                    ),
                                )
                            })
                            .collect()
                    };
                    for (uuid, (sender, receiver)) in senders_and_receivers {
                        if !bidding_unfinished.contains(&uuid) {
                            continue;
                        }
                        let mut receiver_locked = receiver.lock().await;
                        let msg = receiver_locked.try_recv();
                        drop(receiver_locked);
                        if msg.is_err() {
                            let err = msg.err().unwrap();
                            match err {
                                TryRecvError::Empty => continue,
                                TryRecvError::Disconnected => {
                                    let mut game_state = app_state.game_state.write().await;
                                    game_state.unregister_player(&uuid).await;
                                }
                            }
                            continue;
                        }
                        let msg = msg.unwrap();
                        match msg {
                            PlayerToServerMessage::SendBidding { bidding } => {
                                let state = app_state.game_state.read().await;
                                let player = state.players.get(&uuid).unwrap();
                                if bidding < player.action_points {
                                    state
                                        .send_to(
                                            &uuid,
                                            ServerToPlayerMessage::BiddingResult {
                                                bidding,
                                                error: true,
                                                reason: Some(BiddingError::NoEnoughActionPoints {
                                                    need: bidding,
                                                }),
                                            },
                                        )
                                        .await;
                                    continue;
                                }
                                let (bidding_max, bidding_min) = (
                                    app_state.cfg.game_rules.bidding.bid_max,
                                    app_state.cfg.game_rules.bidding.bid_min,
                                );
                                if bidding >= bidding_min && bidding <= bidding_max {
                                    if app_state.cfg.game_rules.bidding.broadcast_bid_message {
                                        state
                                            .broadcast(ServerBroadcastMessage::OthersBidding {
                                                uuid,
                                                bidding,
                                            })
                                            .await;
                                    }
                                    player_bidding.insert(uuid, bidding);
                                    bidding_unfinished.remove(&uuid);
                                    state
                                        .send_to(
                                            &uuid,
                                            ServerToPlayerMessage::BiddingResult {
                                                bidding,
                                                error: false,
                                                reason: None,
                                            },
                                        )
                                        .await;
                                } else {
                                    state
                                        .send_to(
                                            &uuid,
                                            ServerToPlayerMessage::BiddingResult {
                                                bidding,
                                                error: true,
                                                reason: Some(BiddingError::BiddingNotValid {
                                                    max: bidding_max,
                                                    min: bidding_min,
                                                }),
                                            },
                                        )
                                        .await;
                                    continue;
                                }
                            }
                            _ => {
                                sender.send(msg).await.unwrap();
                            }
                        }
                    }
                }
                let mut tmp_player_bidding: Vec<(_, _)> = player_bidding.iter().collect();
                tmp_player_bidding.sort_by(|x, y| y.1.cmp(x.1));
                player_bidding = tmp_player_bidding
                    .into_iter()
                    .map(|(x, y)| (*x, *y))
                    .collect();
                if app_state.cfg.game_rules.bidding.broadcast_bid_message {
                    let state = app_state.game_state.read().await;
                    state
                        .broadcast(ServerBroadcastMessage::BiddingSorted {
                            order: player_bidding.keys().cloned().collect(),
                        })
                        .await;
                }
            } else if cur_phase == 3 {
                let mut take_count: HashMap<Items, u32> = HashMap::new();
                take_count.insert(Items::Ore, 0);
                take_count.insert(Items::Wood, 0);
                take_count.insert(Items::Diamond, 0);
                take_count.insert(Items::Gold, 0);
                take_count.insert(Items::Iron, 0);
                take_count.insert(Items::Food, 0);
                let state = app_state.game_state.read().await;
                if !app_state.cfg.game_rules.bidding.enable {
                    continue;
                }
                state
                    .broadcast(ServerBroadcastMessage::PhaseChanged {
                        epoch: cur_epoch,
                        phase: cur_phase,
                    })
                    .await;
                drop(state);
                let senders_and_receivers: Vec<_> = {
                    let game_state = app_state.game_state.read().await;
                    player_bidding
                        .iter()
                        .map(|(x, _y)| {
                            let sender = game_state
                                .players
                                .get(x)
                                .unwrap()
                                .from_channel
                                .sender
                                .clone();
                            let receiver = game_state
                                .players
                                .get(x)
                                .unwrap()
                                .from_channel
                                .receiver
                                .clone();
                            (*x, (sender, receiver))
                        })
                        .collect()
                };
                for (uuid, (sender, receiver)) in senders_and_receivers {
                    if !player_bidding.contains_key(&uuid) {
                        continue;
                    }
                    let state = app_state.game_state.read().await;
                    state
                        .send_to(
                            &uuid,
                            ServerToPlayerMessage::DataRequired {
                                epoch: cur_epoch,
                                phase: cur_phase,
                            },
                        )
                        .await;
                    drop(state);
                    let mut receiver_locked = receiver.lock().await;
                    loop {
                        let msg = receiver_locked.try_recv();
                        if msg.is_err() {
                            let err = msg.err().unwrap();
                            match err {
                                TryRecvError::Empty => continue,
                                TryRecvError::Disconnected => {
                                    let mut game_state = app_state.game_state.write().await;
                                    game_state.unregister_player(&uuid).await;
                                }
                            }
                            continue;
                        }
                        let msg = msg.unwrap();
                        match msg {
                            PlayerToServerMessage::SendContending { action } => match action {
                                ContendingAction::Take { item, index } => {
                                    let state = app_state.game_state.read().await;
                                    let player = state.players.get(&uuid).unwrap();
                                    let bidding = *player_bidding.get(&uuid).unwrap();
                                    if player.action_points < bidding {
                                        state
                                            .send_to(
                                                &uuid,
                                                ServerToPlayerMessage::ContendingResult {
                                                    action,
                                                    error: true,
                                                    reason: Some(
                                                        ContendingError::NoEnoughActionPoints {
                                                            need: bidding,
                                                        },
                                                    ),
                                                },
                                            )
                                            .await;
                                        continue;
                                    }
                                    if state.market.get(index).unwrap() != &item {
                                        state
                                            .send_to(
                                                &uuid,
                                                ServerToPlayerMessage::ContendingResult {
                                                    action,
                                                    error: true,
                                                    reason: Some(ContendingError::ItemNotFound {}),
                                                },
                                            )
                                            .await;
                                        continue;
                                    }
                                    drop(state);
                                    let mut state = app_state.game_state.write().await;
                                    state.market.remove(index);
                                    let player = state.players.get_mut(&uuid).unwrap();
                                    *player.resources.get_mut(&item).unwrap() += 1;
                                    player.action_points -= player_bidding.get(&uuid).unwrap();
                                    *take_count.get_mut(&item).unwrap() += 1;
                                    if app_state.cfg.game_rules.bidding.broadcast_bid_message {
                                        state
                                            .broadcast(ServerBroadcastMessage::OthersContending {
                                                action: action.clone(),
                                            })
                                            .await;
                                    }
                                    state
                                        .send_to(
                                            &uuid,
                                            ServerToPlayerMessage::ContendingResult {
                                                action,
                                                error: false,
                                                reason: None,
                                            },
                                        )
                                        .await;
                                }
                                ContendingAction::End {} => {
                                    let state = app_state.game_state.read().await;
                                    if app_state.cfg.game_rules.bidding.broadcast_bid_message {
                                        state
                                            .broadcast(ServerBroadcastMessage::OthersContending {
                                                action,
                                            })
                                            .await;
                                    }
                                    player_bidding.remove(&uuid);
                                    break;
                                }
                            },
                            _ => {
                                sender.send(msg).await.unwrap();
                            }
                        }
                    }
                }
                if !app_state.cfg.game_rules.value_changing.enable {
                    let mut state = app_state.game_state.write().await;
                    state.increase_phase().await;
                    continue;
                }
                let (mark_up_when, discount_when, mark_up, discount) = (
                    app_state.cfg.game_rules.value_changing.mark_up_when,
                    app_state.cfg.game_rules.value_changing.discount_when,
                    app_state.cfg.game_rules.value_changing.mark_up,
                    app_state.cfg.game_rules.value_changing.discount,
                );
                let mut state = app_state.game_state.write().await;
                for (x, y) in take_count {
                    if y > mark_up_when && mark_up_when != 0 {
                        let now;
                        {
                            let tmp = state.resource_values.get_mut(&x).unwrap();
                            *tmp += mark_up;
                            now = *tmp;
                        }
                        state
                            .broadcast(ServerBroadcastMessage::ValueChanged { item: x, now })
                            .await;
                    } else if y < discount_when && discount_when != 0 {
                        let now;
                        {
                            let tmp = state.resource_values.get_mut(&x).unwrap();
                            *tmp -= discount;
                            if tmp < &mut 1 {
                                *tmp = 1;
                            }
                            now = *tmp;
                        }
                        state
                            .broadcast(ServerBroadcastMessage::ValueChanged { item: x, now })
                            .await;
                    }
                }
            } else if cur_phase == 4 {
                let mut state = app_state.game_state.write().await;
                if !app_state.cfg.game_rules.events.enable {
                    continue;
                }
                state
                    .broadcast(ServerBroadcastMessage::PhaseChanged {
                        epoch: cur_epoch,
                        phase: cur_phase,
                    })
                    .await;
                let mut events_will_be_chosen = vec![];
                if app_state.cfg.game_rules.events.pirate_attack.enable {
                    events_will_be_chosen.push(Events::PirateAttack);
                }
                if app_state.cfg.game_rules.events.famine.enable {
                    events_will_be_chosen.push(Events::Famine);
                }
                if app_state.cfg.game_rules.events.ap_bonus.enable {
                    events_will_be_chosen.push(Events::ApBonus);
                }
                if app_state.cfg.game_rules.events.crop_bonus.enable {
                    events_will_be_chosen.push(Events::CropBonus);
                }
                let random_number = app_state
                    .rng
                    .clone()
                    .random_range(0..events_will_be_chosen.len());
                let event_chosen = events_will_be_chosen[random_number].clone();
                drop(events_will_be_chosen);
                state
                    .broadcast(ServerBroadcastMessage::EventChosen {
                        event: event_chosen.clone(),
                    })
                    .await;
                match event_chosen {
                    Events::PirateAttack => {
                        for pl in state.players.values_mut() {
                            if pl
                                .buildings
                                .iter()
                                .find(|x| match x {
                                    Building::Cannon => true,
                                    _ => false,
                                })
                                .is_some()
                            {
                                pl.to_channel
                                    .sender
                                    .send(ServerToPlayerMessage::BuildingWorked {
                                        building: Building::Cannon,
                                    })
                                    .await
                                    .unwrap();
                                continue;
                            }
                            let need_gold = app_state.cfg.game_rules.events.pirate_attack.need_gold;
                            let now_gold = pl.resources.get_mut(&Items::Gold).unwrap();
                            if *now_gold > need_gold {
                                *now_gold -= need_gold;
                            } else {
                                pl.buildings = vec![];
                            }
                        }
                    }
                    Events::CropBonus => {
                        for pl in state.players.values_mut() {
                            let building = pl.buildings.iter().find(|x| match x {
                                Building::Farm => true,
                                Building::SuperFarm => true,
                                _ => false,
                            });
                            if building.is_some() {
                                let building = *building.unwrap();
                                pl.to_channel
                                    .sender
                                    .send(ServerToPlayerMessage::BuildingWorked { building })
                                    .await
                                    .unwrap();
                                let now_crop = match building {
                                    Building::Farm => {
                                        app_state
                                            .cfg
                                            .game_rules
                                            .investment
                                            .build
                                            .building_cfg
                                            .farm
                                            .auto_give_food
                                    }
                                    Building::SuperFarm => {
                                        app_state
                                            .cfg
                                            .game_rules
                                            .investment
                                            .build
                                            .building_cfg
                                            .super_farm
                                            .auto_give_food
                                    }
                                    _ => {
                                        unreachable!()
                                    }
                                };
                                let bonus = app_state.cfg.game_rules.events.crop_bonus.bonus;
                                let give_crop = (now_crop as f32 * bonus) as u32;
                                *pl.resources.get_mut(&Items::Food).unwrap() += give_crop;
                                continue;
                            }
                        }
                    }
                    Events::ApBonus => {
                        for pl in state.players.values_mut() {
                            let now_ap = pl.action_points;
                            let bonus = app_state.cfg.game_rules.events.crop_bonus.bonus;
                            let give_ap = (now_ap as f32 * bonus) as u32;
                            pl.action_points += give_ap;
                            continue;
                        }
                    }
                    Events::Famine => {
                        for pl in state.players.values_mut() {
                            let building = pl.buildings.iter().find(|x| match x {
                                Building::Farm => true,
                                Building::SuperFarm => true,
                                _ => false,
                            });
                            if building.is_some() {
                                let building = *building.unwrap();
                                pl.to_channel
                                    .sender
                                    .send(ServerToPlayerMessage::BuildingWorked { building })
                                    .await
                                    .unwrap();
                                continue;
                            }
                            let need_food = app_state.cfg.game_rules.events.famine.need_food;
                            let now_food = pl.resources.get_mut(&Items::Food).unwrap();
                            if *now_food > need_food {
                                *now_food -= need_food;
                            } else {
                                pl.action_points = 0;
                                *now_food = 0;
                            }
                        }
                    }
                }
            }
            let mut game_state = app_state.game_state.write().await;
            game_state.increase_phase().await;
        }
    }
}
pub mod config {
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
    }
    impl EventsCfg {
        fn with_defaults() -> Self {
            Self {
                enable: true,
                pirate_attack: Default::default(),
                famine: Default::default(),
                crop_bonus: Default::default(),
                ap_bonus: Default::default(),
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
}
pub mod enums {
    use crate::structs::{GameStateResponse, PlayerInfoResponse};
    use serde::{Deserialize, Serialize};
    use std::collections::{HashMap, HashSet};
    use uuid::Uuid;

    #[derive(Eq, Hash, PartialEq, Copy, Clone, Debug, Serialize, Deserialize)]
    #[serde(rename_all = "snake_case")]
    pub enum Items {
        Gold,
        Wood,
        Diamond,
        Ore,
        Food,
        Iron,
    }
    impl TryFrom<&'static str> for Items {
        type Error = String;

        fn try_from(value: &'static str) -> Result<Self, Self::Error> {
            let value = value.to_lowercase();
            if value == "gold" {
                Ok(Self::Gold)
            } else if value == "diamond" {
                Ok(Self::Diamond)
            } else if value == "wood" {
                Ok(Self::Wood)
            } else if value == "ore" {
                Ok(Self::Ore)
            } else if value == "iron" {
                Ok(Self::Iron)
            } else if value == "food" {
                Ok(Self::Food)
            } else {
                Err("No such item".into())
            }
        }
    }
    impl Into<&'static str> for &Items {
        fn into(self) -> &'static str {
            match self {
                Items::Gold => "gold",
                Items::Wood => "wood",
                Items::Diamond => "diamond",
                Items::Ore => "ore",
                Items::Food => "food",
                Items::Iron => "iron",
            }
        }
    }
    #[derive(Eq, PartialEq, Hash, Copy, Clone, Debug, Serialize, Deserialize)]
    #[serde(rename_all = "snake_case")]
    pub enum Building {
        Farm,
        SuperFarm,
        Miner,
        SuperMiner,
        Bank,
        Cannon,
        Pickaxe,
        Lumber,
    }
    impl TryFrom<&'static str> for Building {
        type Error = String;

        fn try_from(value: &'static str) -> Result<Self, Self::Error> {
            let value = value.to_lowercase();
            if value == "farm" {
                Ok(Self::Farm)
            } else if value.contains("farm") && value.contains("super") {
                Ok(Self::SuperFarm)
            } else if value == "miner" {
                Ok(Self::Miner)
            } else if value.contains("miner") && value.contains("super") {
                Ok(Self::SuperMiner)
            } else if value == "bank" {
                Ok(Self::Bank)
            } else if value == "cannon" {
                Ok(Self::Cannon)
            } else if value == "pickaxe" {
                Ok(Self::Pickaxe)
            } else if value == "lumber" {
                Ok(Self::Lumber)
            } else {
                Err("no such building".into())
            }
        }
    }
    impl Into<&'static str> for &Building {
        fn into(self) -> &'static str {
            match self {
                Building::Farm => "farm",
                Building::SuperFarm => "super_farm",
                Building::Miner => "miner",
                Building::SuperMiner => "super_miner",
                Building::Bank => "bank",
                Building::Cannon => "cannon",
                Building::Pickaxe => "pickaxe",
                Building::Lumber => "lumber",
            }
        }
    }
    #[derive(Clone, Serialize)]
    #[serde(rename_all = "snake_case")]
    pub enum Events {
        PirateAttack,
        CropBonus,
        ApBonus,
        Famine,
    }
    #[derive(Clone, Deserialize)]
    #[serde(tag = "type", content = "data", rename_all = "snake_case")]
    pub enum PlayerToServerMessage {
        RequestGameState {},
        RequestPlayerInfo { uuid: Uuid },
        SendInvestment { action: InvestmentAction },
        SendBidding { bidding: u32 },
        SendContending { action: ContendingAction },
    }
    #[derive(Clone, Serialize)]
    #[serde(tag = "type", content = "target", rename_all = "snake_case")]
    pub enum ServerToPlayerMessage {
        Broadcast(ServerBroadcastMessage),
        DataRequired {
            epoch: u32,
            phase: u32,
        },
        GameStateResponse {
            state: GameStateResponse,
        },
        PlayerInfoResponse {
            uuid: Uuid,
            player: PlayerInfoResponse,
        },
        UuidNotice {
            uuid: Uuid,
        },
        InvestmentResult {
            action: InvestmentAction,
            error: bool,
            #[serde(skip_serializing_if = "Option::is_none")]
            reason: Option<InvestmentError>,
        },
        BuildingWorked {
            building: Building,
        },
        BiddingResult {
            bidding: u32,
            error: bool,
            #[serde(skip_serializing_if = "Option::is_none")]
            reason: Option<BiddingError>,
        },
        ContendingResult {
            action: ContendingAction,
            error: bool,
            #[serde(skip_serializing_if = "Option::is_none")]
            reason: Option<ContendingError>,
        },
    }
    #[derive(Serialize, Clone)]
    #[serde(tag = "type", content = "target", rename_all = "snake_case")]
    pub enum InvestmentError {
        NoEnoughActionPoints {
            need: u32,
        },
        NoEnoughFood {
            need: u32,
        },
        DontHaveMinerOrSuperMiner {},
        LimitsExceeded {
            limit: u32,
        },
        ActionIsNotEnabled {},
        BuildingIsNotEnabled {},
        NoEnoughMaterials {
            need_items: HashMap<Items, u32>,
            need_buildings: HashMap<Building, u32>,
        },
        NoEnoughOre {
            need: u32,
        },
        NoEnoughItem {
            need: HashMap<Items, u32>,
        },
    }
    #[derive(Serialize, Clone)]
    #[serde(tag = "type", content = "target", rename_all = "snake_case")]
    pub enum BiddingError {
        NoEnoughActionPoints { need: u32 },
        BiddingNotValid { max: u32, min: u32 },
    }
    #[derive(Serialize, Clone)]
    #[serde(tag = "type", content = "target", rename_all = "snake_case")]
    pub enum ContendingError {
        NoEnoughActionPoints { need: u32 },
        ItemNotFound {},
    }
    #[derive(Clone, Serialize)]
    #[serde(tag = "type", content = "target", rename_all = "snake_case")]
    pub enum ServerBroadcastMessage {
        PhaseChanged {
            epoch: u32,
            phase: u32,
        },
        GameStart {},
        HeartBeat {
            state: GameStateResponse,
            interval: u32,
        },
        MarketEmpty {},
        GameOver {
            player_total_value: HashMap<Uuid, u32>,
        },
        BiddingSorted {
            order: HashSet<Uuid>,
        },
        OthersBidding {
            uuid: Uuid,
            bidding: u32,
        },
        OthersContending {
            action: ContendingAction,
        },
        ValueChanged {
            item: Items,
            now: u32,
        },
        EventChosen {
            event: Events,
        },
    }
    #[derive(Serialize, Deserialize, Clone)]
    #[serde(tag = "type", content = "data", rename_all = "snake_case")]
    pub enum InvestmentAction {
        Explore {},
        Exchange {},
        Build { building: Building },
        CrushOre {},
        StoreMoney { item: Items, count: u32 },
        End {},
    }
    #[derive(Serialize, Deserialize, Clone)]
    #[serde(tag = "type", content = "data", rename_all = "snake_case")]
    pub enum ContendingAction {
        Take { index: usize, item: Items },
        End {},
    }
}
pub mod routes {
    use super::structs::{AppState, GameStateResponse, Player, PlayerInfoResponse};
    use crate::enums::{PlayerToServerMessage, ServerToPlayerMessage};
    use axum::extract::ws::{Message, Utf8Bytes, WebSocket};
    use axum::extract::{Path, Request, State, WebSocketUpgrade};
    use axum::http::StatusCode;
    use axum::middleware::Next;
    use axum::response::{IntoResponse, Response};
    use futures::stream::{SplitSink, SplitStream};
    use futures::{SinkExt, StreamExt};
    use std::collections::HashMap;
    use std::sync::Arc;
    use tracing::{debug, error, info};
    use uuid::Uuid;

    fn parse_params(input: &str) -> HashMap<String, String> {
        let mut map = HashMap::new();
        for pair in input.split('&') {
            if pair.trim().is_empty() {
                continue;
            }
            let mut parts = pair.splitn(2, '=');
            let key = parts.next().unwrap().trim();
            let value = parts.next().unwrap_or("").trim();
            if !key.is_empty() {
                map.insert(key.to_string(), value.to_string());
            }
        }
        map
    }
    pub async fn auth_middleware(
        state: State<Arc<AppState>>,
        request: Request,
        next: Next,
    ) -> Result<Response, StatusCode> {
        let provided_header = request
            .headers()
            .get("Authorization")
            .and_then(|header| header.to_str().ok());
        let provided_query = request.uri().query().unwrap_or("");
        let provided_query = parse_params(provided_query);
        let provided_query = provided_query.get("token").map(|s| s.as_str());
        let provided_auth = {
            if provided_query.is_some() {
                provided_query
            } else if provided_header.is_some() {
                provided_header
            } else {
                None
            }
        };
        let (token, use_token) = {
            let cfg = state.cfg.clone();
            (cfg.server.token.clone(), cfg.server.use_token)
        };
        if token == "__set_token_here__" || !use_token {
            return Ok(next.run(request).await);
        }
        if let Some(provided_auth) = provided_auth {
            if provided_auth != token {
                return Err(StatusCode::FORBIDDEN);
            }
        } else {
            return Err(StatusCode::UNAUTHORIZED);
        }
        Ok(next.run(request).await)
    }

    pub async fn root() -> &'static str {
        "You are all set!"
    }
    pub async fn ws_handler(
        State(state): State<Arc<AppState>>,
        Path(player_name): Path<String>,
        ws: WebSocketUpgrade,
    ) -> impl IntoResponse {
        let player_obj = {
            let cfg_temp = state.cfg.clone();
            Player::with_cfg(player_name.clone().leak(), &*cfg_temp)
        };
        let uuid = {
            let mut state_temp = state.game_state.write().await;
            state_temp.register_player(player_obj).await
        };
        ws.on_upgrade(move |socket| handler_on_upgrade(state, uuid, socket))
    }
    async fn handler_on_upgrade(state: Arc<AppState>, uuid: Uuid, socket: WebSocket) {
        let (mut writer, reader) = socket.split();
        info!("有玩家加入，分配UUID：{:?}", uuid);
        let msg = ServerToPlayerMessage::UuidNotice { uuid };
        let send_result = writer
            .send(Message::Text(Utf8Bytes::from(
                serde_json::to_string(&msg).unwrap().as_str(),
            )))
            .await;
        if send_result.is_err() {
            return;
        }
        let reader = tokio::spawn(handler_reader(state.clone(), uuid.clone(), reader));
        let writer = tokio::spawn(handler_writer(state.clone(), uuid.clone(), writer));
        tokio::select! {
            _ = reader => {}
            _ = writer => {}
        }
        state
            .game_state
            .write()
            .await
            .unregister_player(&uuid)
            .await;
    }
    async fn handler_reader(state: Arc<AppState>, uuid: Uuid, mut reader: SplitStream<WebSocket>) {
        while let Some(Ok(msg)) = reader.next().await {
            debug!("接收到消息");
            match msg {
                Message::Text(msg) => {
                    let str = msg.to_string();
                    let msg = serde_json::from_str::<PlayerToServerMessage>(str.as_str());
                    match msg {
                        Ok(msg) => {
                            if let PlayerToServerMessage::RequestGameState {} = msg.clone() {
                                let sender = {
                                    let game_state = state.game_state.read().await;
                                    game_state
                                        .players
                                        .get(&uuid.clone())
                                        .unwrap()
                                        .to_channel
                                        .sender
                                        .clone()
                                };
                                let resp = {
                                    let game_state = state.game_state.read().await;
                                    GameStateResponse::from(&*game_state)
                                };
                                let resp = ServerToPlayerMessage::GameStateResponse { state: resp };
                                if sender.send(resp).await.is_err() {
                                    break;
                                }
                                continue;
                            } else if let PlayerToServerMessage::RequestPlayerInfo {
                                uuid: requested_uuid,
                            } = msg.clone()
                            {
                                let sender = {
                                    let game_state = state.game_state.read().await;
                                    game_state
                                        .players
                                        .get(&uuid.clone())
                                        .unwrap()
                                        .to_channel
                                        .sender
                                        .clone()
                                };
                                let resp = {
                                    let game_state = state.game_state.read().await;
                                    let player = game_state.players.get(&requested_uuid);
                                    if player.is_none() {
                                        continue;
                                    }
                                    PlayerInfoResponse::from(&*player.unwrap())
                                };
                                let resp = ServerToPlayerMessage::PlayerInfoResponse {
                                    uuid: uuid.clone(),
                                    player: resp,
                                };
                                if sender.send(resp).await.is_err() {
                                    break;
                                }
                            }
                            let sender = {
                                let game_state = state.game_state.read().await;
                                game_state
                                    .players
                                    .get(&uuid.clone())
                                    .unwrap()
                                    .from_channel
                                    .sender
                                    .clone()
                            };
                            let snd = sender.send(msg).await;
                            if snd.is_err() {
                                break;
                            }
                        }
                        Err(obj) => {
                            error!("json反序列化出错：{:?}", obj);
                        }
                    }
                }
                Message::Close(_) => break,
                _ => {}
            }
        }
    }
    async fn handler_writer(
        state: Arc<AppState>,
        uuid: Uuid,
        mut writer: SplitSink<WebSocket, Message>,
    ) {
        let receiver = {
            let game_state = state.game_state.read().await;
            let player = game_state.players.get(&uuid).unwrap();
            player.to_channel.receiver.clone()
        };

        loop {
            let msg = {
                let mut rx = receiver.lock().await;
                rx.recv().await
            };

            if let Some(msg) = msg {
                debug!("准备发送消息");
                let send_result = writer
                    .send(Message::Text(Utf8Bytes::from(
                        serde_json::to_string(&msg).unwrap().as_str(),
                    )))
                    .await;
                if send_result.is_err() {
                    break;
                }
            } else {
                break;
            }
        }
    }
}
pub mod structs {
    use crate::config::GameCfg;
    use crate::enums::{
        Building, Items, PlayerToServerMessage, ServerBroadcastMessage, ServerToPlayerMessage,
    };
    use rand_chacha::ChaCha20Rng;
    use rand_chacha::rand_core::RngCore;
    use serde::Serialize;
    use std::collections::HashMap;
    use std::sync::Arc;
    use tokio::sync::{Mutex, RwLock};
    use tracing::trace;
    use uuid::Uuid;

    #[derive(Serialize, Clone)]
    pub struct GameStateResponse {
        pub players: Vec<Uuid>,
        pub market: Vec<&'static str>,
        pub epoch: u32,
        pub phase: u32,
        pub values: HashMap<&'static str, u32>,
        pub started: bool,
    }
    impl From<&GameState> for GameStateResponse {
        fn from(value: &GameState) -> Self {
            trace!("{:?}", value.market);
            let market = value
                .market
                .iter()
                .map(|x| x.into())
                .collect::<Vec<&'static str>>();
            let epoch = value.epoch;
            let phase = value.phase;
            let started = value.started;
            let values = value
                .resource_values
                .iter()
                .map(|(item, &v)| (item.into(), v))
                .collect::<HashMap<&'static str, u32>>();
            let players = value.players.keys().cloned().collect::<Vec<Uuid>>();
            Self {
                players,
                market,
                epoch,
                phase,
                values,
                started,
            }
        }
    }
    impl GameStateResponse {
        pub fn with_error() -> GameStateResponse {
            Default::default()
        }
    }
    impl Default for GameStateResponse {
        fn default() -> Self {
            GameStateResponse {
                players: Vec::new(),
                market: Vec::new(),
                epoch: 0,
                phase: 0,
                values: HashMap::new(),
                started: false,
            }
        }
    }
    #[derive(Serialize, Clone)]
    pub struct PlayerInfoResponse {
        name: &'static str,
        action_points: u32,
        resources: HashMap<&'static str, u32>,
        buildings: Vec<&'static str>,
        bank_money: u32,
    }
    impl From<&Player> for PlayerInfoResponse {
        fn from(value: &Player) -> Self {
            let action_points = value.action_points;
            let resources = value
                .resources
                .iter()
                .map(|(x, y)| {
                    let item_str: &'static str = x.into();
                    (item_str, *y)
                })
                .collect::<HashMap<&'static str, u32>>();
            let buildings = value
                .buildings
                .iter()
                .map(|x| x.into())
                .collect::<Vec<&'static str>>();
            let bank_money = value.bank_money;
            Self {
                name: value.name,
                action_points,
                resources,
                buildings,
                bank_money,
            }
        }
    }
    impl PlayerInfoResponse {
        pub fn with_error() -> PlayerInfoResponse {
            Default::default()
        }
    }
    impl Default for PlayerInfoResponse {
        fn default() -> Self {
            PlayerInfoResponse {
                name: "",
                action_points: 0,
                resources: HashMap::new(),
                buildings: Vec::new(),
                bank_money: 0,
            }
        }
    }
    pub struct AppState {
        pub cfg: Arc<GameCfg>,
        pub game_state: Arc<RwLock<GameState>>,
        pub rng: ChaCha20Rng,
    }
    impl AppState {
        pub fn new(cfg: GameCfg, game_state: GameState, rng: ChaCha20Rng) -> AppState {
            Self {
                cfg: Arc::new(cfg),
                game_state: Arc::new(RwLock::new(game_state)),
                rng,
            }
        }
    }
    pub struct Channel<T> {
        pub sender: tokio::sync::mpsc::Sender<T>,
        pub receiver: Arc<Mutex<tokio::sync::mpsc::Receiver<T>>>,
    }
    impl<T> Channel<T> {
        pub fn new() -> Channel<T> {
            let (s, r) = tokio::sync::mpsc::channel(255);
            Channel {
                sender: s,
                receiver: Arc::new(Mutex::new(r)),
            }
        }
    }
    pub struct Player {
        pub name: &'static str,
        pub resources: HashMap<Items, u32>,
        pub action_points: u32,
        pub buildings: Vec<Building>,
        pub bank_money: u32,
        pub from_channel: Channel<PlayerToServerMessage>,
        pub to_channel: Channel<ServerToPlayerMessage>,
    }
    impl Player {
        pub fn new(player_name: &'static str) -> Player {
            let mut res = Self {
                name: player_name,
                resources: HashMap::new(),
                action_points: 0,
                buildings: Vec::new(),
                bank_money: 0,
                from_channel: Channel::new(),
                to_channel: Channel::new(),
            };
            res.resources.insert(Items::Gold, 0);
            res.resources.insert(Items::Iron, 0);
            res.resources.insert(Items::Wood, 0);
            res.resources.insert(Items::Diamond, 0);
            res.resources.insert(Items::Ore, 0);
            res.resources.insert(Items::Food, 0);
            res
        }
        pub fn with_cfg(player_name: &'static str, cfg: &GameCfg) -> Player {
            let mut res = Self::new(player_name);
            cfg.game_rules
                .prepare
                .defaults_give_player
                .apply_to_player(&mut res);
            res
        }
    }
    pub struct GameState {
        pub players: HashMap<Uuid, Player>,
        pub market: Vec<Items>,
        pub current_deck: Vec<Items>,
        pub epoch: u32,
        pub phase: u32,
        pub resource_values: HashMap<Items, u32>,
        pub started: bool,
    }
    impl GameState {
        pub fn new() -> GameState {
            let mut res = GameState {
                players: HashMap::new(),
                market: Vec::new(),
                current_deck: Vec::new(),
                epoch: 1,
                phase: 1,
                resource_values: HashMap::new(),
                started: false,
            };
            res.resource_values.insert(Items::Diamond, 8);
            res.resource_values.insert(Items::Gold, 6);
            res.resource_values.insert(Items::Wood, 2);
            res.resource_values.insert(Items::Ore, 3);
            res.resource_values.insert(Items::Food, 1);
            res.resource_values.insert(Items::Iron, 2);
            res
        }
        async fn apply_configurations(&mut self, conf: &GameCfg) {
            self.resource_values = (&conf.game_rules.resource_values_default.clone()).into();
        }
        pub async fn initialize(&mut self, conf: &GameCfg, rng: &mut ChaCha20Rng) {
            self.apply_configurations(conf).await;
            let deck: HashMap<Items, u32> = (&conf.game_rules.prepare.deck).into();
            deck.iter().for_each(|(x, y)| {
                for _ in 0..*y {
                    self.current_deck.push(x.clone())
                }
            });
            let sze = self.current_deck.len();
            for i in 0..sze {
                let index = rng.next_u32() % (sze as u32);
                let tmp = self.current_deck[i];
                self.current_deck[i] = self.current_deck[index as usize];
                self.current_deck[index as usize] = tmp;
            }
            let mut cards: Vec<Items> = self
                .current_deck
                .drain(0..conf.game_rules.prepare.draw_cards as usize)
                .collect();
            self.market.append(&mut cards);
        }
        pub async fn broadcast(&self, message: ServerBroadcastMessage) {
            for (_, player) in self.players.iter() {
                let _ = player
                    .to_channel
                    .sender
                    .send(ServerToPlayerMessage::Broadcast { 0: message.clone() })
                    .await;
            }
        }
        pub async fn send_to(&self, uuid: &Uuid, message: ServerToPlayerMessage) {
            self.players
                .get(uuid)
                .unwrap()
                .to_channel
                .sender
                .send(message)
                .await
                .unwrap();
        }
        pub async fn register_player(&mut self, player: Player) -> Uuid {
            loop {
                let uuid = Uuid::new_v4();
                if !self.players.contains_key(&uuid) {
                    self.players.insert(uuid.clone(), player);
                    break uuid;
                }
            }
        }
        pub async fn unregister_player(&mut self, uuid: &Uuid) {
            if let Some(player) = self.players.remove(uuid) {
                drop(player);
            }
        }
        pub async fn increase_phase(&mut self) {
            self.phase += 1;
            if self.phase == 5 {
                self.epoch += 1;
                self.phase = 0;
            }
        }
    }
    impl Default for GameState {
        fn default() -> Self {
            Self::new()
        }
    }
}
