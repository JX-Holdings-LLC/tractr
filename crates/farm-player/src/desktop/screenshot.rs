//! F12 screenshots: the frame on screen as a PNG in the game's screenshots folder
//! (`<data dir>/<gameId>/screenshots/`), named after the time it was taken (UTC).

use std::fs;
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};

/// The key that takes a screenshot (farm-runtime key name).
pub const KEY: &str = "f12";

/// `YYYY-MM-DD_HH-MM-SS` of a Unix time (UTC): sorts by time and is valid on every file system.
fn stamp(unix: i64) -> String {
    let seconds = unix.rem_euclid(86_400);
    let (hours, minutes, seconds) = (seconds / 3600, seconds / 60 % 60, seconds % 60);
    format!("{}_{hours:02}-{minutes:02}-{seconds:02}", farm_ui::format::utc_date(unix))
}

/// Writes `png` to a new file in `folder` (created when missing): `screenshot-<time>.png`, with a
/// counter when that name is taken (two screenshots in the same second). Returns its path.
pub fn write(folder: &Path, unix: i64, png: &[u8]) -> std::io::Result<PathBuf> {
    fs::create_dir_all(folder)?;
    let stamp = stamp(unix);
    for attempt in 0..100u32 {
        let name = match attempt {
            0 => format!("screenshot-{stamp}.png"),
            _ => format!("screenshot-{stamp}-{attempt}.png"),
        };
        let path = folder.join(name);
        // `create_new` never replaces an existing file or writes through a link.
        match fs::OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(mut file) => {
                file.write_all(png)?;
                return Ok(path);
            }
            Err(error) if error.kind() == ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }
    Err(std::io::Error::new(ErrorKind::AlreadyExists, "too many screenshots in one second"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_follow_the_time() {
        assert_eq!(stamp(0), "1970-01-01_00-00-00");
        assert_eq!(stamp(1_792_603_503), "2026-10-21_17-25-03");
    }

    #[test]
    fn screenshots_in_the_same_second_get_their_own_files() {
        let folder = std::env::temp_dir().join(format!("farm-screenshots-{}", std::process::id()));
        let _ = fs::remove_dir_all(&folder);
        let first = write(&folder, 0, b"one").unwrap();
        let second = write(&folder, 0, b"two").unwrap();
        assert!(first.ends_with("screenshot-1970-01-01_00-00-00.png"), "{first:?}");
        assert!(second.ends_with("screenshot-1970-01-01_00-00-00-1.png"), "{second:?}");
        assert_eq!(fs::read(&first).unwrap(), b"one");
        assert_eq!(fs::read(&second).unwrap(), b"two");
        let _ = fs::remove_dir_all(&folder);
    }
}
