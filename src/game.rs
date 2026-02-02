use crate::enums::{
    BiddingError, Building, ContendingAction, ContendingError, Events, InvestmentAction,
    InvestmentError, Items, PlayerToServerMessage, ServerBroadcastMessage, ServerToPlayerMessage,
};
use crate::structs::{AppState, GameStateResponse};
use crate::utils::{discount_recipe, parse_recipe, verify_recipe};
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
                                let ore_index = current_deck.iter().position(|x| x == &Items::Wood);
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
                        PlayerToServerMessage::SendInvestment { action } => {
                            match action {
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
                            }
                        }
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
