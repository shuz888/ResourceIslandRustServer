#[cfg(test)]
mod tests {
    use resource_island_server::enums::Items;

    #[tokio::test]
    async fn test_01_borrowed_str_into_item_enum() {
        assert_eq!(Items::Diamond, "diamond".try_into().unwrap());
        assert_eq!(Items::Ore, "ore".try_into().unwrap());
        assert_eq!(Items::Wood, "wood".try_into().unwrap());
        assert_eq!(Items::Iron, "iron".try_into().unwrap());
        assert_eq!(Items::Food, "food".try_into().unwrap());
        assert_eq!(Items::Gold, "gold".try_into().unwrap());
        Items::try_from("hey").unwrap_err();
    }

    #[tokio::test]
    async fn test_02_items_enum_into_borrowed_str() {
        use resource_island_server::enums::Items;
        assert_eq!(
            "diamond",
            <&Items as Into<&'static str>>::into((&Items::Diamond).into())
        );
        assert_eq!(
            "ore",
            <&Items as Into<&'static str>>::into((&Items::Ore).into())
        );
        assert_eq!(
            "wood",
            <&Items as Into<&'static str>>::into((&Items::Wood).into())
        );
        assert_eq!(
            "gold",
            <&Items as Into<&'static str>>::into((&Items::Gold).into())
        );
        assert_eq!(
            "food",
            <&Items as Into<&'static str>>::into((&Items::Food).into())
        );
        assert_eq!(
            "iron",
            <&Items as Into<&'static str>>::into((&Items::Iron).into())
        );
    }

    #[tokio::test]
    async fn test_03_borrowed_str_into_building_enum() {
        use resource_island_server::enums::Building;
        assert_eq!(Building::Bank, "bank".try_into().unwrap());
        assert_eq!(Building::Farm, "farm".try_into().unwrap());
        assert_eq!(Building::SuperFarm, "super_farm".try_into().unwrap());
        assert_eq!(Building::Miner, "miner".try_into().unwrap());
        assert_eq!(Building::SuperMiner, "super_miner".try_into().unwrap());
        assert_eq!(Building::Cannon, "cannon".try_into().unwrap());
        Building::try_from("hey").unwrap_err();
    }
}
