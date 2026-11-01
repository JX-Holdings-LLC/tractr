//! The balancing lab (#230) on the starter farm's cartridge: deterministic, and it farms.

use farm_bench::lab::{run_lab, Strategy};

const CART: &[u8] = include_bytes!("../../../fixtures/golden/cartridges/starter-farm.cart");

#[test]
fn the_lab_farms_reproducibly_and_reports() {
    let cart = farm_cart::load_cartridge(CART).unwrap();
    let crop = cart
        .content
        .crops
        .keys()
        .find(|id| {
            cart.content
                .items
                .iter()
                .any(|item| item.r#type == "seed" && item.crop_type.as_deref() == Some(id.as_str()))
        })
        .cloned()
        .expect("a crop with a seed");
    let strategies = [Strategy::Crop(crop.clone()), Strategy::BestCrop, Strategy::Process];
    let seeds = ["a".to_owned(), "b".to_owned()];
    let first = run_lab(&cart, &strategies, &seeds, 12);
    let second = run_lab(&cart, &strategies, &seeds, 12);
    assert_eq!(first, second);
    assert_eq!(first.runs.len(), 6);

    let single = &first.runs[0];
    assert_eq!(single.days.len(), 12);
    let row = &single.crops[&crop];
    assert!(row.planted > 0, "{row:?}");
    assert!(row.energy > 0, "{row:?}");
    assert!(single.days.iter().any(|day| day.spent > 0));

    let csv = first.csv();
    assert_eq!(csv.len(), 6);
    assert!(csv[0].1.starts_with("content_hash,strategy,seed"));
    assert_eq!(csv[1].1.lines().count(), 1 + 6 * 12);
    assert!(first.summary().contains("best-crop"));
}
