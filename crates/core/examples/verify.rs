//! Runs the image check on real images, to set its thresholds.
//!
//! ```text
//! cargo run --release -p mtg-art-core --example verify -- <art crop> <candidate>...
//! ```
//!
//! Prints, for each candidate, the correlation, how much of it the art crop
//! covers, and the verdict. Keep the images outside the repo: they're
//! artists' work.

use std::{path::Path, process::ExitCode};

use mtg_art_core::verify::{
    analyse, compare_images, FRAME_CONTRAST, MATCH_SCORE, MIN_GAIN, MIN_HEIGHT_COVERED, MIN_WIDTH_COVERED,
};

fn main() -> ExitCode {
    let paths: Vec<String> = std::env::args().skip(1).collect();
    let [crop, candidates @ ..] = paths.as_slice() else {
        eprintln!("usage: verify <art crop> <candidate>...");
        return ExitCode::FAILURE;
    };
    let crop = match open(crop) {
        Ok(image) => image,
        Err(error) => {
            eprintln!("{crop}: {error}");
            return ExitCode::FAILURE;
        }
    };
    println!(
        "thresholds: score {MATCH_SCORE}, width {MIN_WIDTH_COVERED}, height {MIN_HEIGHT_COVERED}, frame {FRAME_CONTRAST}, gain {MIN_GAIN}; art crop {}x{}",
        crop.width(),
        crop.height()
    );
    for path in candidates {
        let name = Path::new(path).file_name().map_or(path.into(), |n| n.to_string_lossy());
        match open(path) {
            Ok(candidate) => {
                let verdict = compare_images(&crop, &candidate);
                match analyse(&crop, &candidate) {
                    Some(a) => println!(
                        "{:.3}  w {:.2}  h {:.2}  f {}  {}x{}  {verdict:?}  {name}",
                        a.score,
                        a.width_covered,
                        a.height_covered,
                        a.frame_contrast.map_or("-".into(), |f| format!("{f:.2}")),
                        a.width,
                        a.height
                    ),
                    None => println!("  -    {verdict:?}  {name}"),
                }
            }
            Err(error) => println!("  !    {error}  {name}"),
        }
    }
    ExitCode::SUCCESS
}

/// Opens an image by what's in it, whatever its name.
fn open(path: &str) -> image::ImageResult<image::DynamicImage> {
    image::ImageReader::open(path)?.with_guessed_format()?.decode()
}
