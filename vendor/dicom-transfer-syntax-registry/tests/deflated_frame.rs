//! Test suite for Deflated Image Frame Compression pixel data reading and writing
#![cfg(feature = "deflate")]

mod adapters;

use adapters::TestDataObject;
use dicom_core::value::PixelFragmentSequence;
use dicom_encoding::{
    Codec,
    adapters::{EncodeOptions, PixelDataReader, PixelDataWriter},
};
use dicom_transfer_syntax_registry::entries::DEFLATED_IMAGE_FRAME_COMPRESSION;

fn gen_rgb_samples(rows: u16, columns: u16) -> Vec<u8> {
    // build some random RGB image
    let mut samples = vec![0; rows as usize * columns as usize * 3];

    // use linear congruence to make RGB noise
    let mut seed = 0xcfcf_acab_u32;
    let mut gen_sample = || {
        let r = 4_294_967_291_u32;
        let b = 67291_u32;
        seed = seed.wrapping_mul(r).wrapping_add(b);
        // grab a portion from the seed
        (seed >> 7) as u8
    };

    let slab = 8;
    for y in (0..rows as usize).step_by(slab) {
        let scan_r = gen_sample();
        let scan_g = gen_sample();
        let scan_b = gen_sample();

        for x in 0..columns as usize {
            for k in 0..slab {
                let offset = ((y + k) * columns as usize + x) * 3;
                samples[offset] = scan_r;
                samples[offset + 1] = scan_g;
                samples[offset + 2] = scan_b;
            }
        }
    }

    samples
}

/// writing to Deflated Image Frame Compression and back
/// should yield exactly the same pixel data
#[test]
fn write_and_read_deflated_frames() {
    let rows: u16 = 128;
    let columns: u16 = 256;

    // build some random RGB image
    let samples = gen_rgb_samples(rows, columns);

    // create test object of native encoding
    let obj = TestDataObject {
        // Explicit VR Little Endian
        ts_uid: "1.2.840.10008.1.2.1".to_string(),
        rows,
        columns,
        bits_allocated: 8,
        bits_stored: 8,
        samples_per_pixel: 3,
        photometric_interpretation: "RGB",
        number_of_frames: 1,
        flat_pixel_data: Some(samples.clone()),
        pixel_data_sequence: None,
    };

    // fetch adapters for Deflated Image Frame Compression

    let Codec::EncapsulatedPixelData(Some(reader), Some(writer)) =
        DEFLATED_IMAGE_FRAME_COMPRESSION.codec()
    else {
        panic!("Deflated Image Frame Compression pixel data adapters not found")
    };

    let mut encoded = vec![];

    let _ops = writer
        .encode_frame(&obj, 0, EncodeOptions::default(), &mut encoded)
        .expect("Deflated Image Frame encoding failed");

    // instantiate new object representing the compressed version

    let obj = TestDataObject {
        // Deflated Image Frame Compression
        ts_uid: "1.2.840.10008.1.2.8.1".to_string(),
        rows,
        columns,
        bits_allocated: 8,
        bits_stored: 8,
        samples_per_pixel: 3,
        photometric_interpretation: "RGB",
        number_of_frames: 1,
        flat_pixel_data: None,
        pixel_data_sequence: Some(PixelFragmentSequence::new(vec![], vec![encoded])),
    };

    // decode frame
    let mut decoded = vec![];

    reader
        .decode_frame(&obj, 0, &mut decoded)
        .expect("Deflated Image Frame decoding failed");

    // compare pixels, lossless encoding should yield exactly the same data
    assert_eq!(samples, decoded, "pixel data mismatch");
}

/// An object encoded to Deflated Image Frame Compression
/// produces a suitable basic offset table.
#[test]
fn encode_whole_object_offset_table() {
    let rows = 256;
    let columns = 256;

    let mut samples = Vec::new();
    // generate 4 frames
    for _ in 0..4 {
        samples.extend(gen_rgb_samples(rows, columns));
    }
    let samples = samples;

    // create test object of native encoding
    let obj = TestDataObject {
        // Explicit VR Little Endian
        ts_uid: "1.2.840.10008.1.2.1".to_string(),
        rows,
        columns,
        bits_allocated: 8,
        bits_stored: 8,
        samples_per_pixel: 3,
        photometric_interpretation: "RGB",
        number_of_frames: 4,
        flat_pixel_data: Some(samples.clone()),
        pixel_data_sequence: None,
    };

    // fetch adapter
    let Codec::EncapsulatedPixelData(_, Some(writer)) = DEFLATED_IMAGE_FRAME_COMPRESSION.codec()
    else {
        panic!("Deflated Image Compression pixel data adapters not found")
    };

    let mut encoded = Vec::new();
    let mut offset_table = Vec::new();

    let _ops = writer
        .encode(
            &obj,
            EncodeOptions::default(),
            &mut encoded,
            &mut offset_table,
        )
        .expect("image encoding failed");

    // expect 4 fragments, 1 per frame
    assert_eq!(encoded.len(), 4);

    // expect these items in offset table
    assert_eq!(
        &offset_table,
        &[
            // fragment 0
            0,
            // fragment 1
            encoded[0].len() as u32 + 8,
            // fragment 2
            encoded[0].len() as u32 + encoded[1].len() as u32 + 8 * 2,
            // fragment 3
            encoded[0].len() as u32 + encoded[1].len() as u32 + encoded[2].len() as u32 + 8 * 3,
        ]
    );
}
