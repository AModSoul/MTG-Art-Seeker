//! Whether an image found elsewhere is the art Scryfall has for a card face,
//! and larger.
//!
//! Scryfall's art crop is cut from the middle of a card's art box, and the art
//! box is itself a crop of the painting, so a larger copy of the art usually
//! has more around the edges. A whole-image comparison won't match it; the
//! crop is looked for inside the candidate instead, by normalised
//! cross-correlation on small grayscale copies of both: first across every
//! scale on rough copies, then around the best spot on finer ones.
//!
//! Where the crop sits in the candidate also tells a card scan from the art.
//! On a card, the art is a window in the frame, spanning most of the card's
//! width but less than half its height. That alone would also turn away a
//! portrait painting a borderless card was cut from, so a match that small
//! only counts as a card when the picture also stops at its edges: a scan's
//! art meets the frame along a hard line, where a painting carries on.

use std::path::Path;

use image::{imageops::FilterType, DynamicImage, GrayImage, ImageReader, Limits};

use crate::{Error, Result};

/// The lowest correlation counted as the same picture. On 40 real pairs,
/// the same art scored 0.83 and up (the lowest were scans of 2000-era cards)
/// and other art 0.49 at most.
pub const MATCH_SCORE: f32 = 0.7;
/// How much of a candidate's width and height the art crop has to cover for
/// the candidate to be the art for certain. Less, and it's the art in a
/// card's frame if [`FRAME_CONTRAST`] says the picture stops at the match.
pub const MIN_WIDTH_COVERED: f32 = 0.6;
pub const MIN_HEIGHT_COVERED: f32 = 0.7;
/// How much more the image changes across the match's edges than it does
/// anywhere else, for the match to be a window in a frame. On 40 real pairs,
/// card scans measured 1.57 to 5.2, and portrait paintings 1.0 to 1.52.
pub const FRAME_CONTRAST: f32 = 2.0;
/// The same, for a card-shaped image. A scan is card-shaped, and so is a
/// painting for a borderless card, but such paintings measured 1.06 at most.
pub const CARD_FRAME_CONTRAST: f32 = 1.35;
/// A card's width over its height (63 × 88 mm), and how far from it an image
/// can be and still be card-shaped.
const CARD_ASPECT: f32 = 63.0 / 88.0;
const CARD_ASPECT_TOLERANCE: f32 = 0.03;
/// How much wider than Scryfall's art crop a copy has to be to count as
/// larger: a few pixels more is no better.
pub const MIN_GAIN: f32 = 1.1;

/// The widths the candidate is searched at: roughly, then finely.
const COARSE_WIDTH: u32 = 64;
const FINE_WIDTH: u32 = 192;
/// The art crop's width as a share of the candidate's, from smallest to
/// largest, searched roughly.
const SCALES: [f32; 10] = [0.55, 0.6, 0.65, 0.7, 0.75, 0.8, 0.85, 0.9, 0.95, 1.0];
/// The same for a split card's half ([`CropPart`]), which is only part of
/// the half's art crop, so takes less of the painting.
const PART_SCALES: [f32; 15] = [
    0.3, 0.35, 0.4, 0.45, 0.5, 0.55, 0.6, 0.65, 0.7, 0.75, 0.8, 0.85, 0.9, 0.95, 1.0,
];
/// Where the part of a split card's art crop that's only its second half's
/// painting starts, as a share of the crop's width. The first half takes from
/// half (Fire // Ice) to about 61% (an aftermath card's) of it.
const SECOND_HALF_FROM: f32 = 0.65;
/// How far either side of the match's edges the frame test looks, in pixels
/// of the fine copy.
const FRAME_REACH: u32 = 2;
/// How far around the rough match the fine search looks.
const FINE_SCALE_STEP: f32 = 0.01;
const FINE_SCALE_STEPS: i32 = 3;
const FINE_OFFSET: i32 = 4;

/// The largest image decoded, so a hostile file can't exhaust memory.
const MAX_SIDE: u32 = 16_384;
const MAX_ALLOCATION: u64 = 512 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// The same art, larger than Scryfall's.
    Same { width: u32, height: u32 },
    /// The same art, or something like it, but no larger than Scryfall's.
    Smaller,
    /// The art inside a card's frame: a scan or a render of the card.
    CardScan,
    /// Not this art.
    Different,
}

/// How the art crop was found in a candidate.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Analysis {
    /// The best normalised cross-correlation, from -1 to 1.
    pub score: f32,
    /// The share of the candidate's width and height the match covers.
    pub width_covered: f32,
    pub height_covered: f32,
    /// The candidate's own size, and the match's width in its pixels.
    pub width: u32,
    pub height: u32,
    pub match_width: u32,
    /// How much more the image changes across the match's edges than it does
    /// elsewhere; `None` when the match fills the candidate.
    pub frame_contrast: Option<f32>,
}

impl Analysis {
    pub fn verdict(&self, art_crop_width: u32) -> Verdict {
        let small = self.width_covered < MIN_WIDTH_COVERED || self.height_covered < MIN_HEIGHT_COVERED;
        let card_shaped = (self.width as f32 / self.height as f32 - CARD_ASPECT).abs() <= CARD_ASPECT_TOLERANCE;
        let threshold = if card_shaped {
            CARD_FRAME_CONTRAST
        } else {
            FRAME_CONTRAST
        };
        let framed = self.frame_contrast.is_some_and(|contrast| contrast >= threshold);
        if self.score < MATCH_SCORE {
            Verdict::Different
        } else if small && framed {
            Verdict::CardScan
        } else if (self.match_width as f32) < art_crop_width as f32 * MIN_GAIN {
            Verdict::Smaller
        } else {
            Verdict::Same {
                width: self.width,
                height: self.height,
            }
        }
    }
}

/// How much better the other order has to match for a split card's halves'
/// paintings to count as the wrong way round.
const SWAP_MARGIN: f32 = 0.1;

/// Which of a face's art Scryfall's art crop shows. A split card's crop shows
/// both halves side by side, the first on the left.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CropPart {
    Whole,
    /// A split card's first half: the crop's left half, which is all its.
    FirstHalf,
    /// A split card's second half: the crop from [`SECOND_HALF_FROM`] on.
    SecondHalf,
}

/// What a candidate is checked against: the art crop, or for a split card's
/// half, the part of it that's that half's painting. Decoded once, to check
/// any number of candidates.
pub struct Reference {
    image: DynamicImage,
    scales: &'static [f32],
}

impl Reference {
    pub fn new(art_crop: &DynamicImage, part: CropPart) -> Self {
        let (width, height) = (art_crop.width(), art_crop.height());
        match part {
            CropPart::Whole => Self {
                image: art_crop.clone(),
                scales: &SCALES,
            },
            CropPart::FirstHalf => Self {
                image: art_crop.crop_imm(0, 0, (width / 2).max(1), height),
                scales: &PART_SCALES,
            },
            CropPart::SecondHalf => {
                let from = ((width as f32 * SECOND_HALF_FROM) as u32).min(width.saturating_sub(1));
                Self {
                    image: art_crop.crop_imm(from, 0, width - from, height),
                    scales: &PART_SCALES,
                }
            }
        }
    }

    /// The art crop at `path`, decoded.
    pub fn open(path: &Path, part: CropPart) -> Result<Self> {
        Ok(Self::new(&open(path)?, part))
    }

    pub fn compare(&self, candidate: &DynamicImage) -> Verdict {
        // A copy no wider than the crop can't hold a larger copy of it.
        if candidate.width() <= self.image.width() {
            return Verdict::Smaller;
        }
        match analyse_at(&self.image, candidate, self.scales) {
            Some(analysis) => analysis.verdict(self.image.width()),
            None => Verdict::Different,
        }
    }

    /// Compares the image at `candidate`.
    pub fn compare_file(&self, candidate: &Path) -> Result<Verdict> {
        Ok(self.compare(&open(candidate)?))
    }
}

/// Whether the paintings saved for a split card's halves, `first` and
/// `second`, are the wrong way round. Scryfall's art crop of a split card
/// shows its halves side by side, the first on the left, though not always
/// half and half: an aftermath card's first half takes more of it. Each half
/// of the crop is looked for in each painting, and the order that matches
/// clearly better wins; when neither does, they're left as they are.
pub fn halves_swapped(art_crop: &Path, first: &Path, second: &Path) -> Result<bool> {
    Ok(halves_swapped_images(&open(art_crop)?, &open(first)?, &open(second)?))
}

pub fn halves_swapped_images(art_crop: &DynamicImage, first: &DynamicImage, second: &DynamicImage) -> bool {
    let (width, height) = (art_crop.width(), art_crop.height());
    let left = art_crop.crop_imm(0, 0, width / 2, height);
    let right = art_crop.crop_imm(width / 2, 0, width - width / 2, height);
    let score = |part: &DynamicImage, painting: &DynamicImage| analyse(part, painting).map_or(0.0, |found| found.score);
    let as_saved = score(&left, first) + score(&right, second);
    let swapped = score(&left, second) + score(&right, first);
    swapped > as_saved + SWAP_MARGIN
}

/// Compares `candidate` with the whole of `art_crop`.
pub fn compare_images(art_crop: &DynamicImage, candidate: &DynamicImage) -> Verdict {
    Reference::new(art_crop, CropPart::Whole).compare(candidate)
}

/// Where the art crop best matches inside `candidate`; `None` if either is
/// blank or the crop's shape can't fit in it.
pub fn analyse(art_crop: &DynamicImage, candidate: &DynamicImage) -> Option<Analysis> {
    analyse_at(art_crop, candidate, &SCALES)
}

/// [`analyse`], trying the crop at `scales` of the candidate's width.
fn analyse_at(art_crop: &DynamicImage, candidate: &DynamicImage, scales: &[f32]) -> Option<Analysis> {
    let crop = art_crop.to_luma8();
    let aspect = crop.height() as f32 / crop.width() as f32;
    let candidate_gray = candidate.to_luma8();

    // Roughly: every scale, every position.
    let coarse = shrink(&candidate_gray, COARSE_WIDTH);
    let mut best: Option<(f32, f32, u32, u32)> = None;
    for &scale in scales {
        if let Some((score, x, y)) = best_position(&coarse, &crop, scale, aspect, None) {
            if best.map_or(true, |(s, ..)| score > s) {
                best = Some((score, scale, x, y));
            }
        }
    }
    let (_, rough_scale, rough_x, rough_y) = best?;

    // Finely: near that scale, and near that spot scaled up.
    let fine = shrink(&candidate_gray, FINE_WIDTH);
    let ratio = fine.width() as f32 / coarse.width() as f32;
    let near = ((rough_x as f32 * ratio) as i32, (rough_y as f32 * ratio) as i32);
    let mut best: Option<(f32, u32, u32, u32)> = None;
    for step in -FINE_SCALE_STEPS..=FINE_SCALE_STEPS {
        let scale = rough_scale + step as f32 * FINE_SCALE_STEP;
        if !(0.0..=1.0).contains(&scale) {
            continue;
        }
        if let Some((score, x, y)) = best_position(&fine, &crop, scale, aspect, Some(near)) {
            let template_width = (scale * fine.width() as f32).round() as u32;
            if best.map_or(true, |(s, ..)| score > s) {
                best = Some((score, template_width, x, y));
            }
        }
    }
    let (score, template_width, x, y) = best?;
    let template_height = (template_width as f32 * aspect).round() as u32;
    let frame_contrast = frame_contrast(&fine, x, y, template_width, template_height);
    let match_width = (template_width as f32 / fine.width() as f32 * candidate.width() as f32).round() as u32;
    Some(Analysis {
        score,
        width_covered: template_width as f32 / fine.width() as f32,
        height_covered: template_height as f32 / fine.height() as f32,
        width: candidate.width(),
        height: candidate.height(),
        match_width,
        frame_contrast,
    })
}

/// How much more `image` changes across the edges of the rectangle at
/// `(x, y)` than it does between any two pixels as far apart. Edges that lie
/// on the image's own border are left out; `None` if they all do.
fn frame_contrast(image: &GrayImage, x: u32, y: u32, width: u32, height: u32) -> Option<f32> {
    let reach = FRAME_REACH;
    let gap = 2 * reach;
    let at = |x: u32, y: u32| f32::from(image.get_pixel(x, y).0[0]);
    let (right, bottom) = (x + width, y + height);
    let mut across = Vec::new();
    // Left and right edges: a pixel `reach` inside against one `reach` outside.
    for row in y..bottom.min(image.height()) {
        if x >= reach && x + reach < image.width() {
            across.push((at(x + reach, row) - at(x - reach, row)).abs());
        }
        if right >= reach && right + reach < image.width() {
            across.push((at(right - reach, row) - at(right + reach, row)).abs());
        }
    }
    // Top and bottom edges.
    for col in x..right.min(image.width()) {
        if y >= reach && y + reach < image.height() {
            across.push((at(col, y + reach) - at(col, y - reach)).abs());
        }
        if bottom >= reach && bottom + reach < image.height() {
            across.push((at(col, bottom - reach) - at(col, bottom + reach)).abs());
        }
    }
    if across.is_empty() {
        return None;
    }
    // Every pair of pixels as far apart, across the whole image.
    let mut total = 0.0;
    let mut count = 0u32;
    for row in 0..image.height() {
        for col in 0..image.width() {
            if col + gap < image.width() {
                total += (at(col + gap, row) - at(col, row)).abs();
                count += 1;
            }
            if row + gap < image.height() {
                total += (at(col, row + gap) - at(col, row)).abs();
                count += 1;
            }
        }
    }
    let typical = (total / count.max(1) as f32).max(1.0);
    Some(across.iter().sum::<f32>() / across.len() as f32 / typical)
}

/// Decodes an image, with limits on its size.
fn open(path: &Path) -> Result<DynamicImage> {
    let invalid = |reason: String| Error::Invalid {
        url: path.display().to_string(),
        reason,
    };
    let mut reader = ImageReader::open(path)?
        .with_guessed_format()
        .map_err(|e| invalid(e.to_string()))?;
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_SIDE);
    limits.max_image_height = Some(MAX_SIDE);
    limits.max_alloc = Some(MAX_ALLOCATION);
    reader.limits(limits);
    reader.decode().map_err(|e| invalid(e.to_string()))
}

/// `image` scaled to `width` wide, keeping its shape.
fn shrink(image: &GrayImage, width: u32) -> GrayImage {
    let height = ((image.height() as f32 * width as f32 / image.width() as f32).round() as u32).max(1);
    image::imageops::resize(image, width, height, FilterType::Triangle)
}

/// The crop at `scale` of `haystack`'s width: its best correlation, and where.
/// `near` limits the search to [`FINE_OFFSET`] pixels around a spot.
fn best_position(
    haystack: &GrayImage,
    crop: &GrayImage,
    scale: f32,
    aspect: f32,
    near: Option<(i32, i32)>,
) -> Option<(f32, u32, u32)> {
    let width = (scale * haystack.width() as f32).round() as u32;
    let height = (width as f32 * aspect).round() as u32;
    if width < 4 || height < 4 || width > haystack.width() || height > haystack.height() {
        return None;
    }
    let template = Template::new(&image::imageops::resize(crop, width, height, FilterType::Triangle))?;
    let (max_x, max_y) = (haystack.width() - width, haystack.height() - height);
    let (xs, ys) = match near {
        Some((x, y)) => (clamp_range(x, FINE_OFFSET, max_x), clamp_range(y, FINE_OFFSET, max_y)),
        None => (0..=max_x, 0..=max_y),
    };
    let mut best: Option<(f32, u32, u32)> = None;
    for y in ys {
        for x in xs.clone() {
            if let Some(score) = template.correlate(haystack, x, y) {
                if best.map_or(true, |(s, ..)| score > s) {
                    best = Some((score, x, y));
                }
            }
        }
    }
    best
}

fn clamp_range(centre: i32, reach: i32, max: u32) -> std::ops::RangeInclusive<u32> {
    let low = (centre - reach).clamp(0, max as i32) as u32;
    let high = (centre + reach).clamp(0, max as i32) as u32;
    low..=high
}

/// A template with its mean taken out, ready to correlate.
struct Template {
    width: u32,
    height: u32,
    /// Each pixel less the mean.
    centred: Vec<f32>,
    /// The square root of the sum of `centred`'s squares.
    norm: f32,
}

impl Template {
    fn new(image: &GrayImage) -> Option<Self> {
        let pixels: Vec<f32> = image.pixels().map(|p| f32::from(p.0[0])).collect();
        let mean = pixels.iter().sum::<f32>() / pixels.len() as f32;
        let centred: Vec<f32> = pixels.iter().map(|p| p - mean).collect();
        let norm = centred.iter().map(|c| c * c).sum::<f32>().sqrt();
        // A flat template matches everything equally: nothing to go on.
        (norm > f32::EPSILON).then_some(Self {
            width: image.width(),
            height: image.height(),
            centred,
            norm,
        })
    }

    /// The normalised cross-correlation with the window of `haystack` at
    /// `(x, y)`; `None` if the window is flat.
    fn correlate(&self, haystack: &GrayImage, x: u32, y: u32) -> Option<f32> {
        let n = (self.width * self.height) as f32;
        let mut sum = 0.0;
        let mut sum_sq = 0.0;
        let mut dot = 0.0;
        let mut i = 0;
        for row in 0..self.height {
            for col in 0..self.width {
                let value = f32::from(haystack.get_pixel(x + col, y + row).0[0]);
                sum += value;
                sum_sq += value * value;
                dot += value * self.centred[i];
                i += 1;
            }
        }
        // Σ(w - mean_w)(t - mean_t) is Σ w(t - mean_t), since Σ(t - mean_t) is 0.
        let window_norm = (sum_sq - sum * sum / n).max(0.0).sqrt();
        (window_norm > f32::EPSILON).then(|| dot / (window_norm * self.norm))
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use image::{ImageFormat, Rgb, RgbImage};

    use super::*;

    /// A made-up painting: a gradient with overlapping ellipses of random
    /// sizes and colours, and some grain. Each seed gives a different picture.
    /// Tests never use real art.
    fn painting(width: u32, height: u32, seed: u32) -> DynamicImage {
        // A small linear congruential generator: the same picture every run.
        let mut state = u64::from(seed).wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
        let mut random = move || {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            (state >> 33) as f32 / (1u64 << 31) as f32
        };
        let mut image = RgbImage::from_fn(width, height, |x, y| {
            let shade = (x + y) as f32 / (width + height) as f32;
            Rgb([
                (40.0 + shade * 120.0) as u8,
                (60.0 + shade * 60.0) as u8,
                (90.0 - shade * 50.0) as u8,
            ])
        });
        for _ in 0..60 {
            let (cx, cy) = (random() * width as f32, random() * height as f32);
            let (rx, ry) = (
                (0.02 + random() * 0.15) * width as f32,
                (0.02 + random() * 0.15) * height as f32,
            );
            let colour = [random() * 255.0, random() * 255.0, random() * 255.0];
            let span = |centre: f32, reach: f32, max: u32| {
                ((centre - reach).max(0.0) as u32)..((centre + reach).ceil() as u32).min(max)
            };
            for y in span(cy, ry, height) {
                for x in span(cx, rx, width) {
                    let (dx, dy) = ((x as f32 - cx) / rx, (y as f32 - cy) / ry);
                    if dx * dx + dy * dy <= 1.0 {
                        image.put_pixel(x, y, Rgb(colour.map(|c| c as u8)));
                    }
                }
            }
        }
        for (x, y, pixel) in image.enumerate_pixels_mut() {
            let grain = ((x.wrapping_mul(2_654_435_761) ^ y.wrapping_mul(40_503)) % 17) as u8;
            pixel.0 = pixel.0.map(|c| c.saturating_add(grain));
        }
        DynamicImage::ImageRgb8(image)
    }

    /// What Scryfall would cut from `art`: the middle, about 626 × 457.
    fn art_crop(art: &DynamicImage) -> DynamicImage {
        let (w, h) = (art.width(), art.height());
        let crop = art.crop_imm(w / 10, h / 9, w * 8 / 10, (w * 8 / 10) * 457 / 626);
        assert!(crop.height() < h, "the crop has to fit");
        crop.resize_exact(626, 457, FilterType::Triangle)
    }

    /// `crop` set in a card's frame, as on a 745 × 1040 scan.
    fn card_scan(crop: &DynamicImage) -> DynamicImage {
        let mut card = painting(745, 1040, 99).to_rgb8();
        for pixel in card.pixels_mut() {
            // A dull frame and text box.
            pixel.0 = pixel.0.map(|c| c / 4 + 90);
        }
        image::imageops::overlay(&mut card, &crop.to_rgb8(), 60, 110);
        DynamicImage::ImageRgb8(card)
    }

    /// Scryfall's art crop of a split card: `first`'s and `second`'s art side
    /// by side, `first_width` of `width` pixels to the first.
    fn split_crop(
        first: &DynamicImage,
        second: &DynamicImage,
        width: u32,
        first_width: u32,
        height: u32,
    ) -> DynamicImage {
        let part = |art: &DynamicImage, part_width: u32| {
            let (w, h) = (art.width(), art.height());
            art.crop_imm(w / 10, h / 10, w * 8 / 10, h * 8 / 10)
                .resize_exact(part_width, height, FilterType::Triangle)
        };
        let mut crop = RgbImage::new(width, height);
        image::imageops::overlay(&mut crop, &part(first, first_width).to_rgb8(), 0, 0);
        image::imageops::overlay(
            &mut crop,
            &part(second, width - first_width).to_rgb8(),
            i64::from(first_width),
            0,
        );
        DynamicImage::ImageRgb8(crop)
    }

    #[test]
    fn tells_a_split_cards_halves_apart() {
        let (fire, ice) = (painting(1000, 680, 11), painting(960, 652, 12));
        // Fire // Ice: half and half.
        let crop = split_crop(&fire, &ice, 808, 404, 280);
        assert!(!halves_swapped_images(&crop, &fire, &ice));
        assert!(halves_swapped_images(&crop, &ice, &fire));
        // Cut // Ribbons, an aftermath card: the first half takes more.
        let (cut, ribbons) = (painting(1200, 446, 13), painting(1200, 705, 14));
        let crop = split_crop(&cut, &ribbons, 1024, 625, 231);
        assert!(!halves_swapped_images(&crop, &cut, &ribbons));
        assert!(halves_swapped_images(&crop, &ribbons, &cut));
    }

    #[test]
    fn checks_a_split_cards_half_against_its_own_part_of_the_crop() {
        let larger = |art: &DynamicImage| art.resize_exact(art.width() * 2, art.height() * 2, FilterType::Triangle);
        let (fire, ice) = (painting(1000, 680, 11), painting(960, 652, 12));
        let crop = split_crop(&fire, &ice, 808, 404, 280);
        let (first, second) = (
            Reference::new(&crop, CropPart::FirstHalf),
            Reference::new(&crop, CropPart::SecondHalf),
        );
        // The whole crop isn't in either half's painting.
        assert_eq!(compare_images(&crop, &larger(&fire)), Verdict::Different);
        assert!(matches!(first.compare(&larger(&fire)), Verdict::Same { .. }));
        assert!(matches!(second.compare(&larger(&ice)), Verdict::Same { .. }));
        assert_eq!(first.compare(&larger(&ice)), Verdict::Different);
        assert_eq!(second.compare(&larger(&fire)), Verdict::Different);

        // An aftermath card's first half takes more of the crop.
        let (cut, ribbons) = (painting(1200, 446, 13), painting(1200, 705, 14));
        let crop = split_crop(&cut, &ribbons, 1024, 625, 231);
        let (first, second) = (
            Reference::new(&crop, CropPart::FirstHalf),
            Reference::new(&crop, CropPart::SecondHalf),
        );
        assert!(matches!(first.compare(&larger(&cut)), Verdict::Same { .. }));
        assert!(matches!(second.compare(&larger(&ribbons)), Verdict::Same { .. }));
        assert_eq!(second.compare(&larger(&cut)), Verdict::Different);
    }

    #[test]
    fn finds_the_art_crop_in_its_uncropped_art() {
        let art = painting(1200, 900, 1);
        let crop = art_crop(&art);
        assert_eq!(
            compare_images(&crop, &art),
            Verdict::Same {
                width: 1200,
                height: 900
            }
        );
        // Larger still, and recompressed as a JPEG.
        let big = art.resize_exact(2400, 1800, FilterType::Triangle);
        let mut jpeg = Vec::new();
        big.write_to(&mut Cursor::new(&mut jpeg), ImageFormat::Jpeg).unwrap();
        let big = image::load_from_memory(&jpeg).unwrap();
        assert!(matches!(compare_images(&crop, &big), Verdict::Same { width: 2400, .. }));
    }

    #[test]
    fn allows_for_a_colour_grade() {
        let art = painting(1200, 900, 2);
        let crop = art_crop(&art);
        let brighter = art.brighten(25).adjust_contrast(10.0);
        assert!(matches!(compare_images(&crop, &brighter), Verdict::Same { .. }));
    }

    #[test]
    fn rejects_a_card_scan() {
        let art = painting(1200, 900, 3);
        let crop = art_crop(&art);
        let scan = card_scan(&crop);
        let analysis = analyse(&crop, &scan).unwrap();
        assert!(analysis.score >= MATCH_SCORE, "{analysis:?}");
        assert_eq!(analysis.verdict(crop.width()), Verdict::CardScan);
    }

    #[test]
    fn accepts_the_portrait_painting_of_a_borderless_card() {
        // Card-shaped, like a scan, with the art crop a band across its
        // middle; but the picture carries on past the crop, so it's the art.
        let painting = painting(1050, 1466, 8);
        let crop = painting
            .crop_imm(90, 520, 870, 870 * 457 / 626)
            .resize_exact(626, 457, FilterType::Triangle);
        let analysis = analyse(&crop, &painting).unwrap();
        assert!(analysis.height_covered < MIN_HEIGHT_COVERED, "{analysis:?}");
        assert_eq!(
            analysis.verdict(crop.width()),
            Verdict::Same {
                width: 1050,
                height: 1466
            }
        );
    }

    #[test]
    fn rejects_other_art() {
        let crop = art_crop(&painting(1200, 900, 4));
        assert_eq!(compare_images(&crop, &painting(1200, 900, 5)), Verdict::Different);
    }

    #[test]
    fn rejects_a_smaller_copy() {
        let art = painting(1200, 900, 6);
        let crop = art_crop(&art);
        let small = art.resize_exact(600, 450, FilterType::Triangle);
        assert_eq!(compare_images(&crop, &small), Verdict::Smaller);
        // Wider than the crop overall, but its copy of the crop isn't.
        let just_wider = art.resize_exact(700, 525, FilterType::Triangle);
        assert_eq!(compare_images(&crop, &just_wider), Verdict::Smaller);
    }

    #[test]
    fn says_nothing_of_a_blank_image() {
        let crop = art_crop(&painting(1200, 900, 7));
        let blank = DynamicImage::ImageRgb8(RgbImage::from_pixel(1200, 900, Rgb([40, 40, 40])));
        assert_eq!(compare_images(&crop, &blank), Verdict::Different);
    }
}
