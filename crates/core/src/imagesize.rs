//! An image's size in pixels, read from its header rather than by decoding it.

use std::{
    fs::File,
    io::{BufReader, Read, Seek, SeekFrom},
    path::Path,
};

use serde::Serialize;
use ts_rs::TS;

use crate::http::ImageKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct ImageSize {
    pub width: u32,
    pub height: u32,
}

/// The size of the image at `path`, or `None` if its header can't be read.
pub fn image_size(path: &Path, kind: ImageKind) -> Option<ImageSize> {
    let mut file = BufReader::new(File::open(path).ok()?);
    match kind {
        ImageKind::Png => png_size(&mut file),
        ImageKind::Jpeg => jpeg_size(&mut file),
    }
}

/// A PNG's first chunk is its header: the 8-byte signature, the chunk's length
/// and type, then the width and height.
fn png_size(file: &mut impl Read) -> Option<ImageSize> {
    let mut head = [0; 24];
    file.read_exact(&mut head).ok()?;
    if &head[12..16] != b"IHDR" {
        return None;
    }
    let width = u32::from_be_bytes(head[16..20].try_into().ok()?);
    let height = u32::from_be_bytes(head[20..24].try_into().ok()?);
    Some(ImageSize { width, height })
}

/// A JPEG is a run of segments, each a marker and a length; the frame header
/// (`SOFn`) holds the size. Metadata such as EXIF comes first, and is skipped
/// over rather than read.
fn jpeg_size(file: &mut (impl Read + Seek)) -> Option<ImageSize> {
    let mut soi = [0; 2];
    file.read_exact(&mut soi).ok()?;
    if soi != [0xFF, 0xD8] {
        return None;
    }
    loop {
        let mut marker = [0; 2];
        file.read_exact(&mut marker).ok()?;
        if marker[0] != 0xFF {
            return None;
        }
        // Padding between segments.
        if marker[1] == 0xFF {
            file.seek(SeekFrom::Current(-1)).ok()?;
            continue;
        }
        // Markers that stand alone, with no length after them.
        if marker[1] == 0x01 || (0xD0..=0xD7).contains(&marker[1]) {
            continue;
        }
        let mut length = [0; 2];
        file.read_exact(&mut length).ok()?;
        let length = u16::from_be_bytes(length);
        let is_frame = matches!(marker[1], 0xC0..=0xCF) && !matches!(marker[1], 0xC4 | 0xC8 | 0xCC);
        if is_frame {
            // Sample precision, then the height and width.
            let mut frame = [0; 5];
            file.read_exact(&mut frame).ok()?;
            let height = u32::from(u16::from_be_bytes([frame[1], frame[2]]));
            let width = u32::from(u16::from_be_bytes([frame[3], frame[4]]));
            return Some(ImageSize { width, height });
        }
        // The image data has begun without a frame header.
        if marker[1] == 0xDA || length < 2 {
            return None;
        }
        file.seek(SeekFrom::Current(i64::from(length) - 2)).ok()?;
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    #[test]
    fn reads_a_png_header() {
        let mut png = b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR".to_vec();
        png.extend(1488u32.to_be_bytes());
        png.extend(2079u32.to_be_bytes());
        assert_eq!(
            png_size(&mut Cursor::new(png)),
            Some(ImageSize {
                width: 1488,
                height: 2079
            })
        );
        assert_eq!(png_size(&mut Cursor::new(b"\x89PNG".to_vec())), None);
    }

    #[test]
    fn reads_a_jpeg_frame_header_past_its_metadata() {
        let mut jpeg = vec![0xFF, 0xD8];
        // An APP1 (EXIF) segment of 300 bytes, then padding.
        jpeg.extend([0xFF, 0xE1, 0x01, 0x2C]);
        jpeg.extend([0; 298]);
        jpeg.extend([0xFF, 0xFF]);
        // A progressive frame header: precision, 1536 high, 2048 wide.
        jpeg.extend([0xFF, 0xC2, 0x00, 0x11, 0x08, 0x06, 0x00, 0x08, 0x00]);
        assert_eq!(
            jpeg_size(&mut Cursor::new(jpeg)),
            Some(ImageSize {
                width: 2048,
                height: 1536
            })
        );
    }

    #[test]
    fn gives_up_on_a_jpeg_without_a_frame_header() {
        let jpeg = vec![0xFF, 0xD8, 0xFF, 0xDA, 0x00, 0x02];
        assert_eq!(jpeg_size(&mut Cursor::new(jpeg)), None);
        assert_eq!(jpeg_size(&mut Cursor::new(b"<html>".to_vec())), None);
    }
}
