use std::process::Command;

#[test]
fn executable_loads_the_compiled_cartridge() {
    let cart = [env!("CARGO_MANIFEST_DIR"), "..", "..", "fixtures", "golden", "cartridges", "fixture-project.cart"]
        .iter()
        .collect::<std::path::PathBuf>();
    let output = Command::new(env!("CARGO_BIN_EXE_farm-player"))
        .args(["--headless", "--cart", cart.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["gameId"], "local.project-1");
    assert!(report["stateHash"].as_str().is_some_and(|hash| hash.len() == 16));

    let replay = cart.with_file_name("twenty-ticks.json");
    let replayed = Command::new(env!("CARGO_BIN_EXE_farm-player"))
        .args(["--headless", "--cart", cart.to_str().unwrap(), "--replay", replay.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(replayed.status.success(), "{}", String::from_utf8_lossy(&replayed.stderr));
    let replay_report: serde_json::Value = serde_json::from_slice(&replayed.stdout).unwrap();
    assert_ne!(report["stateHash"], replay_report["stateHash"]);
}

#[test]
fn start_options_put_the_new_game_somewhere() {
    let cart = [env!("CARGO_MANIFEST_DIR"), "..", "..", "fixtures", "golden", "cartridges", "fixture-project.cart"]
        .iter()
        .collect::<std::path::PathBuf>();
    let run = |extra: &[&str]| {
        let output = Command::new(env!("CARGO_BIN_EXE_farm-player"))
            .args(["--headless", "--cart", cart.to_str().unwrap()])
            .args(extra)
            .output()
            .unwrap();
        (output.status.success(), output.stdout, String::from_utf8_lossy(&output.stderr).into_owned())
    };
    let (ok, stdout, stderr) = run(&[]);
    assert!(ok, "{stderr}");
    let report: serde_json::Value = serde_json::from_slice(&stdout).unwrap();
    let scene = report["clock"]["sceneId"].as_str().unwrap().to_owned();

    let (ok, stdout, stderr) =
        run(&["--start-scene", &scene, "--start-at", "0,0", "--facing", "left", "--time", "9:30", "--day", "3"]);
    assert!(ok, "{stderr}");
    let report: serde_json::Value = serde_json::from_slice(&stdout).unwrap();
    assert_eq!(report["clock"]["day"], 3);
    assert_eq!(report["clock"]["minute"], 570);
    assert_eq!((report["clock"]["x"].as_i64(), report["clock"]["y"].as_i64()), (Some(0), Some(0)));

    let (ok, _, stderr) = run(&["--start-at", "nowhere"]);
    assert!(!ok);
    assert!(stderr.contains("--start-at wants X,Y"), "{stderr}");
}
