//! Farming math — extracted from src/lib/crops.ts, behavior-identical (characterization-tested),
//! with all randomness going through the seeded RNG instead of Math.random. (Port of
//! `Farming/Crops.cs`, engine-core/src/farming/crops.ts.)

use crate::content_builtin;
use crate::schema::{CropDefinition, CustomCropDefinition};
use indexmap::IndexMap;

/// Merge built-in crop definitions with a project's custom crops.
pub fn merge_crop_definitions(custom_crops: &[CropDefinition]) -> IndexMap<String, CropDefinition> {
    let mut merged = content_builtin::crop_definitions();
    for crop in custom_crops {
        merged.insert(crop.id.clone(), crop.clone());
    }
    merged
}

/// `project.customCrops` variant: each one is converted via [`to_crop_definition`].
pub fn merge_custom_crop_definitions(custom_crops: &[CustomCropDefinition]) -> IndexMap<String, CropDefinition> {
    let converted: Vec<CropDefinition> = custom_crops.iter().map(to_crop_definition).collect();
    merge_crop_definitions(&converted)
}

/// CustomCropDefinition → CropDefinition: every key survives (undeclared ones in `extra`).
pub fn to_crop_definition(custom: &CustomCropDefinition) -> CropDefinition {
    CropDefinition {
        id: custom.id.clone(),
        name: custom.name.clone(),
        visual: custom.visual.clone(),
        seed_cost: custom.seed_cost,
        base_harvest_value: custom.base_harvest_value,
        growth_days: custom.growth_days,
        stages: custom.stages,
        seasons: custom.seasons.clone(),
        regrowth_days: custom.regrowth_days,
        can_regrow: custom.can_regrow,
        multi_tile: custom.multi_tile.clone(),
        mutation_chance: custom.mutation_chance,
        yield_min: custom.yield_min,
        yield_max: custom.yield_max,
        harvest_item_id: custom.harvest_item_id.clone(),
        requirements: custom.requirements.clone(),
        extra: custom.extra.clone(),
    }
}

/// CropDefinition → CustomCropDefinition (a pack crop stored as a project custom crop).
pub fn to_custom_crop_definition(crop: &CropDefinition) -> CustomCropDefinition {
    CustomCropDefinition {
        id: crop.id.clone(),
        name: crop.name.clone(),
        visual: crop.visual.clone(),
        seed_cost: crop.seed_cost,
        base_harvest_value: crop.base_harvest_value,
        growth_days: crop.growth_days,
        stages: crop.stages,
        seasons: crop.seasons.clone(),
        regrowth_days: crop.regrowth_days,
        can_regrow: crop.can_regrow,
        multi_tile: crop.multi_tile.clone(),
        mutation_chance: crop.mutation_chance,
        yield_min: crop.yield_min,
        yield_max: crop.yield_max,
        harvest_item_id: crop.harvest_item_id.clone(),
        requirements: crop.requirements.clone(),
        extra: crop.extra.clone(),
    }
}

// ─── Growth and harvest math (port of the rest of Farming/Crops.cs) ─────────────────────────

use crate::rng::RandomSource;
use crate::schema::{crop_qualities, CropMutationDefinition, CropQualityDefinition, Crop, GameContent, Tile};
use crate::units;

/// Test-only: the engine no longer calls it; the tests pin the reference behaviour.
#[cfg(test)]
pub fn is_crop_mature(stage: i64, stages: i64) -> bool {
    stage >= stages - 1
}

/// Returns one of [`crop_qualities`].
pub fn calculate_crop_quality(watered: bool, fertilized: bool, days_without_water: u32) -> String {
    let mut quality_score: i64 = 0;
    if watered {
        quality_score += 2;
    }
    if fertilized {
        quality_score += 3;
    }
    quality_score -= i64::from(days_without_water);

    if quality_score >= 4 {
        return crop_qualities::IRIDIUM.to_owned();
    }
    if quality_score >= 2 {
        return crop_qualities::GOLD.to_owned();
    }
    if quality_score >= 1 {
        return crop_qualities::SILVER.to_owned();
    }
    crop_qualities::NORMAL.to_owned()
}

/// The mutation a harvest rolls, or `None`. Always draws exactly once when a definition is
/// given. The crop's chance is scaled by the quality tier's mutation multiplier (×1.2 silver,
/// ×1.5 gold, ×2 iridium in the built-in tiers) and split between the mutations by their
/// shares, in list order: with the built-in ones a roll under a tenth of it is ancient, under
/// three tenths golden, under all of it giant.
pub fn roll_mutation<'a>(
    definition: Option<&CropDefinition>,
    quality: Option<&CropQualityDefinition>,
    mutations: &'a [CropMutationDefinition],
    rng: &mut dyn RandomSource,
) -> Option<&'a CropMutationDefinition> {
    let definition = definition?;
    // `definition.mutationChance || 0`
    let base_chance = definition.mutation_chance.unwrap_or(0);
    let quality_bonus_milli = u64::from(quality.map_or(units::MILLI_ONE, |tier| tier.mutation_multiplier));
    // Unclamped like v8 (a chance above 1 still splits into tiers).
    let final_chance = u128::from(base_chance) * u128::from(quality_bonus_milli);
    let roll = u128::from(rng.next_u32()) * 1000;

    let mut share: u128 = 0;
    for mutation in mutations {
        share += u128::from(mutation.share);
        if roll * u128::from(units::MILLI_ONE) < final_chance * share {
            return Some(mutation);
        }
    }
    None
}

/// How many crops a harvest yields: the definition's yield range, times the quality tier's and
/// the mutation's yield multipliers (rounded down).
pub fn roll_yield(
    definition: Option<&CropDefinition>,
    quality: Option<&CropQualityDefinition>,
    mutation: Option<&CropMutationDefinition>,
    rng: &mut dyn RandomSource,
) -> u32 {
    let Some(definition) = definition else {
        return 1;
    };
    let base_yield = rng.int(i64::from(definition.yield_min), i64::from(definition.yield_max));
    // Multipliers in thousandths (built-in: quality ×1.1/×1.3/×1.5, mutation ×2/×2.5/×3).
    let quality_bonus = i64::from(quality.map_or(units::MILLI_ONE, |tier| tier.yield_multiplier));
    let mutation_bonus = i64::from(mutation.map_or(units::MILLI_ONE, |mutation| mutation.yield_multiplier));
    let quantity = base_yield.saturating_mul(quality_bonus).saturating_mul(mutation_bonus).div_euclid(1_000_000);
    u32::try_from(quantity.max(0)).unwrap_or(u32::MAX)
}

/// Whether a crop grows in `season` (an unknown crop never does).
pub fn can_grow_in_season(definition: Option<&CropDefinition>, season: &str) -> bool {
    let Some(definition) = definition else {
        return false;
    };
    definition.seasons.iter().any(|entry| entry == season)
}

/// TS `SEASON_ORDER[Math.floor(gameDay / DAYS_PER_SEASON) % SEASON_ORDER.length]`, which reads
/// `undefined` for a negative day. This signature cannot say `undefined`, so a negative day
/// yields `""`; [`try_get_current_season`] keeps the JS shape. The running game reads the
/// configurable calendar (`game_time`), not this fixed 4 × 28-day one.
/// Test-only: the engine no longer calls it; the tests pin the reference behaviour.
#[cfg(test)]
pub fn get_current_season(game_day: i64) -> String {
    try_get_current_season(game_day).unwrap_or_default()
}

/// JS-faithful `getCurrentSeason`: `None` where JS reads `undefined` (negative index).
/// Test-only: the engine no longer calls it; the tests pin the reference behaviour.
#[cfg(test)]
pub fn try_get_current_season(game_day: i64) -> Option<String> {
    let season_count = content_builtin::SEASON_ORDER.len() as i64;
    // JS `%` keeps the dividend's sign; a negative index reads undefined.
    let season_index = game_day.div_euclid(i64::from(content_builtin::DAYS_PER_SEASON)) % season_count;
    usize::try_from(season_index).ok().map(|index| content_builtin::SEASON_ORDER[index].to_owned())
}

/// JS `(gameDay % DAYS_PER_SEASON) + 1` (the remainder keeps the dividend's sign).
/// Test-only: the engine no longer calls it; the tests pin the reference behaviour.
#[cfg(test)]
pub fn get_day_in_season(game_day: i64) -> i64 {
    (game_day % i64::from(content_builtin::DAYS_PER_SEASON)) + 1
}

/// In-game days to maturity for a definition; at least one (validation requires the field).
pub fn crop_growth_days(definition: &CropDefinition) -> u32 {
    definition.growth_days.unwrap_or(0).max(1)
}

/// In-game days between repeat harvests for regrowing crops; one when absent or 0.
pub fn crop_regrowth_days(definition: &CropDefinition) -> u32 {
    definition.regrowth_days.unwrap_or(0).max(1)
}

/// Visual growth stage for the day-based model (0 .. stages-1).
pub fn compute_crop_stage(crop: &Crop, definition: &CropDefinition) -> u32 {
    let growth_days = u64::from(crop_growth_days(definition));
    let days_grown = u64::from(crop.days_grown.unwrap_or(0)).min(growth_days);
    let last = definition.stages.saturating_sub(1);
    // floor(min(1, daysGrown / growthDays) × (stages − 1)); growth days are at least 1.
    let stage = days_grown * u64::from(last) / growth_days;
    u32::try_from(stage).unwrap_or(last).min(last)
}

/// Mature when it has accumulated enough watered days.
pub fn is_crop_mature_by_days(crop: &Crop, definition: &CropDefinition) -> bool {
    if crop.withered == Some(true) {
        return false;
    }
    crop.days_grown.unwrap_or(0) >= crop_growth_days(definition)
}

/// The ids a harvest of `definition` may give, in order: its `harvestItemId` when set; else
/// `crop-{id}` (the editor's convention) and, for a pack crop `pack:local`, `pack:crop-local`
/// (the pack's own item, namespaced the same way).
pub fn harvest_item_candidates(definition: &CropDefinition) -> Vec<String> {
    if let Some(id) = definition.harvest_item_id.as_deref().filter(|id| !id.is_empty()) {
        return vec![id.to_owned()];
    }
    let mut candidates = vec![format!("crop-{}", definition.id)];
    if let Some((pack, local)) =
        definition.id.split_once(':').filter(|(pack, local)| !pack.is_empty() && !local.is_empty())
    {
        candidates.push(format!("{pack}:crop-{local}"));
    }
    candidates
}

/// The item a harvest of `definition` gives (see [`harvest_item_candidates`]); `Err` names the
/// id that was expected when none exists.
pub fn harvest_item<'a>(
    content: &'a GameContent,
    definition: &CropDefinition,
) -> Result<&'a crate::schema::Item, String> {
    let candidates = harvest_item_candidates(definition);
    candidates
        .iter()
        .find_map(|id| content.items.iter().find(|item| item.id == *id))
        .ok_or_else(|| candidates[0].clone())
}

/// The quality a harvest comes out at: the quality the crop was planted with (the second tier,
/// silver, when the soil was fertilized), one tier higher per three farming levels, at most the
/// top tier. An unknown quality counts as the first tier. Deterministic: it draws nothing.
pub fn harvest_quality(tiers: &[CropQualityDefinition], planted_quality: &str, farming_level: u32) -> String {
    let Some(last) = tiers.len().checked_sub(1) else {
        return planted_quality.to_owned();
    };
    let planted = tiers.iter().position(|tier| tier.id == planted_quality).unwrap_or(0);
    let bonus = usize::try_from(farming_level / 3).unwrap_or(usize::MAX);
    tiers[planted.saturating_add(bonus).min(last)].id.clone()
}

/// The quality a crop is planted at: the first tier, or the second (silver) on fertilized soil.
pub fn planted_quality(tiers: &[CropQualityDefinition], fertilized: bool) -> &str {
    let index = usize::from(fertilized);
    tiers.get(index).or_else(|| tiers.first()).map_or(
        if fertilized { crop_qualities::SILVER } else { crop_qualities::NORMAL },
        |tier| tier.id.as_str(),
    )
}

/// Day-based planting (M2+): crops start unwatered — water them or they won't grow.
pub fn create_planted_crop(crop_type: &str, planted_on_day: u32, quality: &str) -> Crop {
    Crop {
        r#type: crop_type.to_owned(),
        planted_on_day: Some(planted_on_day),
        days_grown: Some(0),
        stage: 0,
        watered: false,
        last_watered_day: None,
        quality: quality.to_owned(),
        mutation: None,
        harvest_count: 0,
        days_without_water: 0,
        ..Crop::default()
    }
}

/// Whether a `width`×`height` crop fits with its top-left corner at (x, y): every covered tile
/// is open soil, as single-tile planting needs (no crop, machine, dropped item, active gathering
/// node or collision). Tiles outside the grid (a ragged row too) don't fit.
pub fn can_place_multi_tile_crop(
    tiles: &[Vec<Tile>],
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    plantable: &dyn Fn(&str) -> bool,
) -> bool {
    for dy in 0..i64::from(height) {
        for dx in 0..i64::from(width) {
            let check_x = i64::from(x) + dx;
            let check_y = i64::from(y) + dy;
            let (Ok(row), Ok(column)) = (usize::try_from(check_y), usize::try_from(check_x)) else {
                return false;
            };
            let Some(tile) = tiles.get(row).and_then(|row| row.get(column)) else {
                return false;
            };
            if !plantable(&tile.r#type)
                || tile.crop.is_some()
                || tile.machine.is_some()
                || tile.item.is_some()
                || tile.collision
                || crate::gathering::is_node_active(tile)
            {
                return false;
            }
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stable_json;

    #[test]
    fn custom_crops_override_builtins_by_id() {
        let custom = CustomCropDefinition {
            id: "wheat".to_owned(),
            name: "Blue Wheat".to_owned(),
            ..CustomCropDefinition::default()
        };
        let merged = merge_custom_crop_definitions(std::slice::from_ref(&custom));
        assert_eq!(merged.len(), content_builtin::crop_definitions().len());
        assert_eq!(merged.get_index_of("wheat"), Some(0));
        assert_eq!(merged["wheat"].name, "Blue Wheat");
        // The JSON shapes are identical.
        assert_eq!(stable_json::stringify(&merged["wheat"]), stable_json::stringify(&custom));
        assert_eq!(to_custom_crop_definition(&merged["wheat"]), custom);
    }
}

/// Port of tests/unit/crops.characterization.test.ts (`CropsCharacterizationTests.cs`).
/// src/lib/crops.ts is a thin wrapper over the Core farming math; the wrappers are reproduced
/// here as local helpers (`getCropDefinition(cropType, customCrops)` →
/// [`merge_custom_crop_definitions`], the Math.random-backed `legacyRandom` → a pinned
/// [`RandomSource`]).
#[cfg(test)]
mod characterization_tests {
    use super::*;
    use crate::stable_json;
    use crate::text;
    use crate::units;

    /// TS legacyRandom with Math.random pinned to a fixed roll; counts draws.
    /// The pinned roll is a fraction of 2³², like v8's `Math.random()` value.
    struct PinnedRandom {
        draw: u32,
        draws: usize,
    }

    impl PinnedRandom {
        fn new(roll: f64) -> Self {
            let draw = units::from_authoring::<units::Probability>(roll).min(u64::from(u32::MAX)) as u32;
            Self { draw, draws: 0 }
        }
    }

    impl RandomSource for PinnedRandom {
        fn next_u32(&mut self) -> u32 {
            self.draws += 1;
            self.draw
        }

        fn int(&mut self, min: i64, max: i64) -> i64 {
            self.draws += 1;
            crate::rng::scale_draw(self.draw, max - min + 1) + min
        }
    }

    fn make_custom_crop(id: &str) -> CustomCropDefinition {
        CustomCropDefinition {
            id: id.to_owned(),
            name: "Custom".to_owned(),
            seed_cost: 5,
            base_harvest_value: 100,
            growth_days: Some(2),
            stages: 5,
            seasons: vec!["winter".to_owned()],
            can_regrow: false,
            yield_min: 1,
            yield_max: 1,
            mutation_chance: Some(units::from_authoring::<units::Probability>(0.5)),
            ..CustomCropDefinition::default()
        }
    }

    fn named(id: &str, name: &str) -> CustomCropDefinition {
        CustomCropDefinition { name: name.to_owned(), ..make_custom_crop(id) }
    }

    fn with_value(id: &str, name: &str, base_harvest_value: i64) -> CustomCropDefinition {
        CustomCropDefinition { base_harvest_value, ..named(id, name) }
    }

    fn with_chance(id: &str, mutation_chance: f64) -> CustomCropDefinition {
        let mutation_chance = Some(units::from_authoring::<units::Probability>(mutation_chance));
        CustomCropDefinition { mutation_chance, ..make_custom_crop(id) }
    }

    fn with_seasons(id: &str, seasons: &[&str]) -> CustomCropDefinition {
        CustomCropDefinition { seasons: seasons.iter().map(|s| (*s).to_owned()).collect(), ..make_custom_crop(id) }
    }

    fn get_all_crop_definitions(custom_crops: &[CustomCropDefinition]) -> IndexMap<String, CropDefinition> {
        merge_custom_crop_definitions(custom_crops)
    }

    fn get_crop_definition(crop_type: &str, custom_crops: &[CustomCropDefinition]) -> Option<CropDefinition> {
        get_all_crop_definitions(custom_crops).get(crop_type).cloned()
    }

    fn check_for_mutation(
        crop_type: &str,
        quality: &str,
        roll: f64,
        custom_crops: &[CustomCropDefinition],
    ) -> Option<String> {
        let mutations = content_builtin::crop_mutations();
        roll_mutation(
            get_crop_definition(crop_type, custom_crops).as_ref(),
            tier(quality).as_ref(),
            &mutations,
            &mut PinnedRandom::new(roll),
        )
        .map(|mutation| mutation.id.clone())
    }

    fn tier(quality: &str) -> Option<CropQualityDefinition> {
        content_builtin::crop_qualities().into_iter().find(|tier| tier.id == quality)
    }

    fn mutation(id: Option<&str>) -> Option<CropMutationDefinition> {
        content_builtin::crop_mutations().into_iter().find(|mutation| Some(mutation.id.as_str()) == id)
    }

    fn calculate_yield(crop_type: &str, quality: &str, mutation_id: Option<&str>, roll: f64) -> u32 {
        roll_yield(
            get_crop_definition(crop_type, &[]).as_ref(),
            tier(quality).as_ref(),
            mutation(mutation_id).as_ref(),
            &mut PinnedRandom::new(roll),
        )
    }

    fn grows_in_season(crop_type: &str, season: &str, custom_crops: &[CustomCropDefinition]) -> bool {
        can_grow_in_season(get_crop_definition(crop_type, custom_crops).as_ref(), season)
    }

    fn soil_only(terrain: &str) -> bool {
        terrain == "soil"
    }

    /// h rows by w cols grid of empty soil tiles.
    fn soil_grid(w: usize, h: usize) -> Vec<Vec<Tile>> {
        (0..h).map(|_| (0..w).map(|_| Tile { r#type: "soil".to_owned(), ..Tile::default() }).collect()).collect()
    }

    // --- exported constants ---

    #[test]
    fn built_in_quality_and_mutation_multipliers() {
        let qualities: Vec<(String, u32, u32, u32)> = content_builtin::crop_qualities()
            .into_iter()
            .map(|tier| (tier.id, tier.price_multiplier, tier.yield_multiplier, tier.mutation_multiplier))
            .collect();
        let expected: Vec<(String, u32, u32, u32)> = [
            ("normal", 1000, 1000, 1000),
            ("silver", 1250, 1100, 1200),
            ("gold", 1500, 1300, 1500),
            ("iridium", 2000, 1500, 2000),
        ]
        .into_iter()
        .map(|(id, price, yields, mutation)| (id.to_owned(), price, yields, mutation))
        .collect();
        assert_eq!(qualities, expected);
        let mutations: Vec<(String, u32, u32)> = content_builtin::crop_mutations()
            .into_iter()
            .map(|mutation| (mutation.id, mutation.yield_multiplier, mutation.share))
            .collect();
        let expected: Vec<(String, u32, u32)> = [("ancient", 3000, 100), ("golden", 2500, 200), ("giant", 2000, 700)]
            .into_iter()
            .map(|(id, yields, share)| (id.to_owned(), yields, share))
            .collect();
        assert_eq!(mutations, expected);
    }

    #[test]
    fn days_per_season_is_28() {
        assert_eq!(content_builtin::DAYS_PER_SEASON, 28);
    }

    #[test]
    fn season_order_is_spring_summer_fall_winter() {
        assert_eq!(content_builtin::SEASON_ORDER, &["spring", "summer", "fall", "winter"]);
    }

    #[test]
    fn crop_definitions_contains_exactly_the_9_built_in_crops_keyed_by_id() {
        let definitions = content_builtin::crop_definitions();
        let mut keys: Vec<&str> = definitions.keys().map(String::as_str).collect();
        keys.sort_by(|a, b| text::compare_strings(a, b));
        assert_eq!(
            keys,
            ["blueberry", "carrot", "cauliflower", "corn", "potato", "pumpkin", "strawberry", "tomato", "wheat"]
        );
        for (key, definition) in &definitions {
            assert_eq!(key, &definition.id);
        }
    }

    #[test]
    fn wheat_built_in_definition_is_pinned_exactly() {
        // The 0.01 mutation chance is stored as the threshold round(0.01 × 2³²) = 42949673,
        // written back as its exact value.
        assert_eq!(
            stable_json::stringify(&content_builtin::crop_definitions()["wheat"]),
            r#"{"baseHarvestValue":25,"canRegrow":false,"growthDays":3,"id":"wheat","mutationChance":0.010000000009313226,"name":"Wheat","seasons":["spring","fall"],"seedCost":10,"stages":4,"yieldMax":2,"yieldMin":1}"#
        );
    }

    // --- getAllCropDefinitions / getCropDefinition ---

    #[test]
    fn returns_only_the_9_built_ins_with_no_custom_crops() {
        let all = get_all_crop_definitions(&[]);
        assert_eq!(all.len(), 9);
        assert_eq!(
            stable_json::stringify(&content_builtin::crop_definitions()["wheat"]),
            stable_json::stringify(&all["wheat"])
        );
    }

    #[test]
    fn merges_in_custom_crops_under_their_id() {
        let all = get_all_crop_definitions(&[named("moonberry", "Moonberry")]);
        assert_eq!(all.len(), 10);
        assert_eq!(all["moonberry"].name, "Moonberry");
    }

    #[test]
    fn custom_crop_with_a_built_in_id_overrides_the_built_in_definition() {
        let all = get_all_crop_definitions(&[with_value("wheat", "Evil Wheat", 999)]);
        assert_eq!(all.len(), 9);
        assert_eq!(all["wheat"].name, "Evil Wheat");
        assert_eq!(all["wheat"].base_harvest_value, 999);
    }

    #[test]
    fn get_crop_definition_resolves_built_ins_unknowns_and_overrides() {
        assert_eq!(get_crop_definition("corn", &[]), content_builtin::crop_definitions().get("corn").cloned());
        assert_eq!(get_crop_definition("nope", &[]), None);
        let custom = [make_custom_crop("moonberry"), named("wheat", "Evil Wheat")];
        assert_eq!(get_crop_definition("moonberry", &custom).map(|d| d.id), Some("moonberry".to_owned()));
        assert_eq!(get_crop_definition("wheat", &custom).map(|d| d.name), Some("Evil Wheat".to_owned()));
    }

    #[test]
    fn is_mature_exactly_when_stage_is_at_least_stages_minus_one() {
        assert!(is_crop_mature(3, 4));
        assert!(!is_crop_mature(2, 4));
        assert!(is_crop_mature(4, 4)); // over-shoot still mature
        assert!(is_crop_mature(0, 1));
    }

    // --- calculateCropQuality: score = (watered ? 2 : 0) + (fertilized ? 3 : 0) - daysWithoutWater ---

    #[test]
    fn calculate_crop_quality_boundaries() {
        let cases: [(bool, bool, u32, &str); 11] = [
            (true, true, 0, "iridium"),
            (true, true, 1, "iridium"),
            (true, true, 2, "gold"),
            (true, true, 3, "gold"),
            (true, true, 4, "silver"),
            (true, true, 5, "normal"),
            (true, false, 0, "gold"),
            (true, false, 1, "silver"),
            (true, false, 2, "normal"),
            (false, true, 0, "gold"), // QUIRK: fertilized only caps at gold
            (false, false, 0, "normal"),
        ];
        for (watered, fertilized, days_without_water, expected) in cases {
            assert_eq!(
                calculate_crop_quality(watered, fertilized, days_without_water),
                expected,
                "watered={watered} fertilized={fertilized} days={days_without_water}"
            );
        }
    }

    // --- checkForMutation ---

    #[test]
    fn roll_bands_for_a_half_chance_crop_at_normal_quality() {
        let cases: [(f64, Option<&str>); 9] = [
            (0.0, Some("ancient")),
            (0.0499, Some("ancient")),
            (0.05, Some("golden")), // exact ancient boundary falls through to golden
            (0.1, Some("golden")),
            // The golden boundary 0.15 × 2³² = 644245094.4 is not a draw: 0.15 reads as draw
            // 644245094 (golden), and the first draw past the boundary, 644245095, is giant.
            (0.15, Some("golden")),
            (0.1500000001, Some("giant")),
            (0.4999, Some("giant")),
            (0.5, None), // exact chance boundary is NOT a mutation
            (0.999, None),
        ];
        for (roll, expected) in cases {
            assert_eq!(
                check_for_mutation("mutey", "normal", roll, &[with_chance("mutey", 0.5)]).as_deref(),
                expected,
                "roll={roll}"
            );
        }
    }

    #[test]
    fn quality_bonus_scales_the_chance() {
        // wheat baseChance 0.01
        assert_eq!(check_for_mutation("wheat", "silver", 0.0119, &[]).as_deref(), Some("giant"));
        assert_eq!(check_for_mutation("wheat", "normal", 0.0119, &[]), None);
        assert_eq!(check_for_mutation("wheat", "gold", 0.0149, &[]).as_deref(), Some("giant"));
        assert_eq!(check_for_mutation("wheat", "silver", 0.0149, &[]), None);
        assert_eq!(check_for_mutation("wheat", "iridium", 0.0199, &[]).as_deref(), Some("giant"));
        assert_eq!(check_for_mutation("wheat", "gold", 0.0199, &[]), None);
    }

    #[test]
    fn a_zero_mutation_chance_crop_never_mutates_even_on_roll_0() {
        assert_eq!(check_for_mutation("dud", "iridium", 0.0, &[with_chance("dud", 0.0)]), None);
    }

    #[test]
    fn unknown_crop_type_mutation_returns_null_without_consuming_a_random_roll() {
        let mut rng = PinnedRandom::new(0.0);
        let mutations = content_builtin::crop_mutations();
        assert_eq!(roll_mutation(get_crop_definition("nope", &[]).as_ref(), tier("iridium").as_ref(), &mutations, &mut rng), None);
        assert_eq!(rng.draws, 0);
    }

    // --- calculateYield ---

    #[test]
    fn base_yield_roll_spans_yield_min_to_yield_max_inclusive() {
        let cases: [(&str, f64, u32); 6] = [
            ("wheat", 0.0, 1),
            ("wheat", 0.4999, 1),
            ("wheat", 0.5, 2),
            ("wheat", 0.9999, 2),
            ("blueberry", 0.0, 2),
            ("blueberry", 0.9999, 5),
        ];
        for (crop, roll, expected) in cases {
            assert_eq!(calculate_yield(crop, "normal", None, roll), expected, "crop={crop} roll={roll}");
        }
    }

    #[test]
    fn quality_and_mutation_bonuses_are_floored_and_compound() {
        // quality bonus is floored away on small yields (base 1)
        assert_eq!(calculate_yield("wheat", "silver", None, 0.0), 1);
        assert_eq!(calculate_yield("wheat", "gold", None, 0.0), 1);
        assert_eq!(calculate_yield("wheat", "iridium", None, 0.0), 1);
        // quality bonus applies on larger base yields (base 2)
        assert_eq!(calculate_yield("wheat", "silver", None, 0.9999), 2);
        assert_eq!(calculate_yield("wheat", "gold", None, 0.9999), 2);
        assert_eq!(calculate_yield("wheat", "iridium", None, 0.9999), 3);
        // mutation bonus: giant x2, golden x2.5, ancient x3 (pumpkin base yield always 1)
        assert_eq!(calculate_yield("pumpkin", "normal", Some("giant"), 0.0), 2);
        assert_eq!(calculate_yield("pumpkin", "normal", Some("golden"), 0.0), 2);
        assert_eq!(calculate_yield("pumpkin", "normal", Some("ancient"), 0.0), 3);
        // compound
        assert_eq!(calculate_yield("wheat", "iridium", Some("ancient"), 0.9999), 9);
    }

    #[test]
    fn unknown_crop_type_yield_returns_1_without_consuming_a_random_roll() {
        let mut rng = PinnedRandom::new(0.9999);
        let (quality, ancient) = (tier("iridium"), mutation(Some("ancient")));
        assert_eq!(roll_yield(get_crop_definition("nope", &[]).as_ref(), quality.as_ref(), ancient.as_ref(), &mut rng), 1);
        assert_eq!(rng.draws, 0);
    }

    // --- canGrowInSeason ---

    #[test]
    fn can_grow_in_season_matches_the_definition_seasons_list() {
        assert!(grows_in_season("wheat", "spring", &[]));
        assert!(grows_in_season("wheat", "fall", &[]));
        assert!(!grows_in_season("wheat", "summer", &[]));
        assert!(!grows_in_season("wheat", "winter", &[]));
        assert!(grows_in_season("carrot", "winter", &[]));
        assert!(!grows_in_season("nope", "spring", &[]));
        let custom = [with_seasons("wheat", &["winter"])];
        assert!(!grows_in_season("wheat", "spring", &custom));
        assert!(grows_in_season("wheat", "winter", &custom));
    }

    // --- getCurrentSeason / getDayInSeason ---

    #[test]
    fn game_day_is_zero_based_season_rolls_over_at_day_28() {
        let cases: [(i64, Option<&str>); 10] = [
            (0, Some("spring")),
            (27, Some("spring")),
            (28, Some("summer")),
            (55, Some("summer")),
            (56, Some("fall")),
            (83, Some("fall")),
            (84, Some("winter")),
            (111, Some("winter")),
            (112, Some("spring")), // full year wraps
            (-1, None),            // QUIRK: negative gameDay gives undefined season
        ];
        for (day, season) in cases {
            assert_eq!(try_get_current_season(day).as_deref(), season, "day={day}");
            assert_eq!(get_current_season(day), season.unwrap_or(""), "day={day}");
        }
    }

    #[test]
    fn get_day_in_season_is_one_based_within_a_28_day_season() {
        let cases: [(i64, i64); 6] = [(0, 1), (27, 28), (28, 1), (55, 28), (112, 1), (-1, 0)];
        for (day, expected) in cases {
            // QUIRK: (-1 % 28) + 1 = 0
            assert_eq!(get_day_in_season(day), expected, "day={day}");
        }
    }

    // --- createPlantedCrop / day-based helpers ---

    #[test]
    fn planted_crop_starts_unwatered_on_its_planting_day() {
        assert_eq!(
            stable_json::stringify(&create_planted_crop("wheat", 3, "normal")),
            r#"{"daysGrown":0,"daysWithoutWater":0,"harvestCount":0,"mutation":null,"plantedOnDay":3,"quality":"normal","stage":0,"type":"wheat","watered":false}"#
        );
        assert_eq!(planted_quality(&content_builtin::crop_qualities(), true), "silver");
        assert_eq!(planted_quality(&content_builtin::crop_qualities(), false), "normal");
    }

    #[test]
    fn growth_and_regrowth_days_are_at_least_one() {
        let wheat = &content_builtin::crop_definitions()["wheat"];
        assert_eq!(crop_growth_days(wheat), 3);
        assert_eq!(crop_growth_days(&CropDefinition { growth_days: None, ..wheat.clone() }), 1);
        assert_eq!(crop_growth_days(&CropDefinition { growth_days: Some(0), ..wheat.clone() }), 1);
        let tomato = &content_builtin::crop_definitions()["tomato"];
        assert_eq!(crop_regrowth_days(tomato), 2);
        assert_eq!(crop_regrowth_days(&CropDefinition { regrowth_days: None, ..tomato.clone() }), 1);
        assert_eq!(crop_regrowth_days(&CropDefinition { regrowth_days: Some(0), ..tomato.clone() }), 1);
    }

    #[test]
    fn stage_and_maturity_follow_watered_days() {
        let wheat = &content_builtin::crop_definitions()["wheat"];
        let mut crop = create_planted_crop("wheat", 1, "normal");
        assert_eq!(compute_crop_stage(&crop, wheat), 0);
        assert!(!is_crop_mature_by_days(&crop, wheat));
        crop.days_grown = Some(2);
        assert_eq!(compute_crop_stage(&crop, wheat), 2);
        assert!(!is_crop_mature_by_days(&crop, wheat));
        crop.days_grown = Some(3);
        assert_eq!(compute_crop_stage(&crop, wheat), 3);
        assert!(is_crop_mature_by_days(&crop, wheat));
        crop.days_grown = Some(99);
        assert_eq!(compute_crop_stage(&crop, wheat), 3);
        crop.withered = Some(true);
        assert!(!is_crop_mature_by_days(&crop, wheat));
    }

    // --- canPlaceMultiTileCrop ---

    #[test]
    fn can_place_multi_tile_crop_pins_its_behavior() {
        assert!(can_place_multi_tile_crop(&soil_grid(4, 4), 0, 0, 2, 2, &soil_only));
        assert!(can_place_multi_tile_crop(&soil_grid(4, 4), 2, 2, 2, 2, &soil_only)); // flush with edge
        assert!(!can_place_multi_tile_crop(&soil_grid(4, 4), 3, 0, 2, 2, &soil_only)); // off right edge
        assert!(!can_place_multi_tile_crop(&soil_grid(4, 4), 0, 3, 2, 2, &soil_only)); // off bottom edge
        assert!(!can_place_multi_tile_crop(&soil_grid(4, 4), -1, 0, 2, 2, &soil_only));
        assert!(!can_place_multi_tile_crop(&soil_grid(4, 4), 0, -1, 2, 2, &soil_only));

        let mut occupied = soil_grid(4, 4);
        occupied[1][1].crop = Some(Crop { r#type: "wheat".to_owned(), ..Crop::default() });
        assert!(!can_place_multi_tile_crop(&occupied, 0, 0, 2, 2, &soil_only));
        assert!(can_place_multi_tile_crop(&occupied, 2, 2, 2, 2, &soil_only)); // clear region still fine

        let mut grass = soil_grid(4, 4);
        grass[0][1].r#type = "grass".to_owned();
        assert!(!can_place_multi_tile_crop(&grass, 0, 0, 2, 2, &soil_only));

        // QUIRK: zero-sized footprint is always placeable, even out of bounds
        assert!(can_place_multi_tile_crop(&soil_grid(2, 2), 99, 99, 0, 0, &soil_only));
    }

    #[test]
    fn multi_tile_crops_need_every_covered_tile_open_like_single_tile_planting() {
        let blocked = |edit: &dyn Fn(&mut Tile)| {
            let mut grid = soil_grid(3, 3);
            edit(&mut grid[1][1]);
            !can_place_multi_tile_crop(&grid, 0, 0, 2, 2, &soil_only)
        };
        assert!(blocked(&|tile| tile.machine = Some(crate::schema::TileMachine::default())));
        assert!(blocked(&|tile| tile.item = Some(crate::schema::Item::default())));
        assert!(blocked(&|tile| tile.collision = true));
        assert!(blocked(&|tile| {
            tile.node = Some(crate::schema::TileNode { remaining_health: 3, ..crate::schema::TileNode::default() });
        }));
        // A depleted node (waiting to respawn) doesn't block, as for single tiles.
        assert!(!blocked(&|tile| tile.node = Some(crate::schema::TileNode::default())));
    }

    #[test]
    fn ragged_grids_do_not_fit_a_multi_tile_crop() {
        let soil = || Tile { r#type: "soil".to_owned(), ..Tile::default() };
        let ragged = vec![vec![soil(), soil()], vec![soil()]];
        assert!(!can_place_multi_tile_crop(&ragged, 0, 0, 2, 2, &soil_only));
    }

    #[test]
    fn harvest_quality_starts_at_the_planted_quality_and_rises_with_farming_skill() {
        assert_eq!(harvest_quality(&content_builtin::crop_qualities(), "normal", 0), "normal");
        assert_eq!(harvest_quality(&content_builtin::crop_qualities(), "silver", 2), "silver");
        assert_eq!(harvest_quality(&content_builtin::crop_qualities(), "normal", 3), "silver");
        assert_eq!(harvest_quality(&content_builtin::crop_qualities(), "silver", 3), "gold");
        assert_eq!(harvest_quality(&content_builtin::crop_qualities(), "normal", 6), "gold");
        assert_eq!(harvest_quality(&content_builtin::crop_qualities(), "silver", 9), "iridium");
        assert_eq!(harvest_quality(&content_builtin::crop_qualities(), "bogus", 0), "normal");
    }

    #[test]
    fn ragged_grids_refuse_the_footprint_instead_of_panicking() {
        // The TS threw here (it took the width from the first row).
        let soil = || Tile { r#type: "soil".to_owned(), ..Tile::default() };
        let ragged = vec![vec![soil(), soil()], vec![soil()]];
        assert!(!can_place_multi_tile_crop(&ragged, 0, 0, 2, 2, &soil_only));
        assert!(can_place_multi_tile_crop(&ragged, 0, 0, 1, 2, &soil_only));
    }
}
