#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn test_01_borrowed_str_into_item_enum() {
        use resource_island_server::enums::Items;
        assert_eq!(Items::Diamond, "钻石".try_into().unwrap());
        assert_eq!(Items::Ore, "矿石".try_into().unwrap());
        assert_eq!(Items::Wood, "木材".try_into().unwrap());
        assert_eq!(Items::Iron, "铁".try_into().unwrap());
        assert_eq!(Items::Food, "食物".try_into().unwrap());
        assert_eq!(Items::Gold, "金币".try_into().unwrap());
        Items::try_from("hey").unwrap_err();
    }

    #[tokio::test]
    async fn test_02_items_enum_into_borrowed_str() {
        use resource_island_server::enums::Items;
        assert_eq!(
            "钻石",
            <&Items as Into<&'static str>>::into((&Items::Diamond).into())
        );
        assert_eq!(
            "矿石",
            <&Items as Into<&'static str>>::into((&Items::Ore).into())
        );
        assert_eq!(
            "木材",
            <&Items as Into<&'static str>>::into((&Items::Wood).into())
        );
        assert_eq!(
            "金币",
            <&Items as Into<&'static str>>::into((&Items::Gold).into())
        );
        assert_eq!(
            "食物",
            <&Items as Into<&'static str>>::into((&Items::Food).into())
        );
        assert_eq!(
            "铁",
            <&Items as Into<&'static str>>::into((&Items::Iron).into())
        );
    }

    #[tokio::test]
    async fn test_03_borrowed_str_into_building_enum() {
        use resource_island_server::enums::Building;
        assert_eq!(Building::Bank, "银行".try_into().unwrap());
        assert_eq!(Building::Farm, "农场".try_into().unwrap());
        assert_eq!(Building::SuperFarm, "终极农场".try_into().unwrap());
        assert_eq!(Building::Miner, "矿机".try_into().unwrap());
        assert_eq!(Building::SuperMiner, "超级矿机".try_into().unwrap());
        assert_eq!(Building::Cannon, "炮台".try_into().unwrap());
        Building::try_from("hey").unwrap_err();
    }
}
