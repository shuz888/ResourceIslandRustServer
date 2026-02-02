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
