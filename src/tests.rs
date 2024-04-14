use crate::*;

fn decode_slice_alternative(encoded_ref: &[u8], decoded_ref: &[u8], recoded_ref: &[u8]) {
    let mut encoded_buf = [0u8; 4096];
    let mut decoded_buf = [0u8; 4096];

    let decoded_len = decode_from_slice(encoded_ref, &mut decoded_buf).unwrap();

    let decoded_buf = &decoded_buf[..decoded_len];

    let encoded_len = encode_into_slice(decoded_buf, &mut encoded_buf).unwrap();

    let encoded_buf = &encoded_buf[..encoded_len];

    assert_eq!(encoded_buf, recoded_ref);
    assert_eq!(decoded_buf, decoded_ref);
}

fn decode_slice_round_trip(encoded_ref: &[u8], decoded_ref: &[u8]) {
    decode_slice_alternative(encoded_ref, decoded_ref, encoded_ref)
}

fn encode_slice_round_trip(decoded_ref: &[u8], encoded_ref: &[u8]) {
    let mut encoded_buf = [0u8; 4096];
    let mut decoded_buf = [0u8; 4096];

    let encoded_len = encode_into_slice(decoded_ref, &mut encoded_buf).unwrap();

    let encoded_buf = &encoded_buf[..encoded_len];

    let decoded_len = decode_from_slice(encoded_buf, &mut decoded_buf).unwrap();

    let decoded_buf = &decoded_buf[..decoded_len];

    assert_eq!(decoded_buf, decoded_ref);
    assert_eq!(encoded_buf, encoded_ref);
}

fn decode_slice_with_error<const N: usize>(payload: &[u8], error: DecodeSliceError) {
    assert_eq!(decode_from_slice(payload, &mut [0u8; N]), Err(error));
}

fn encode_slice_with_error<const N: usize>(payload: &[u8], error: EncodeSliceError) {
    assert_eq!(encode_into_slice(payload, &mut [0u8; N]), Err(error));
}

#[test]
fn decode_slice_truncated_input() {
    decode_slice_with_error::<32>(&[0b00111000], DecodeSliceError::TruncatedInput);
}

#[test]
fn decode_slice_needs_more_space() {
    decode_slice_with_error::<3>(
        &[
            0b00000000, 0b00001111, 0b01111111, 0b00000000, 0b00001111, 0b11111111,
        ],
        DecodeSliceError::NeedsMoreSpace,
    );
}

#[test]
fn decode_slice_unaligned() {
    decode_slice_with_error::<32>(
        &[0b00100101, 0b00000000, 0b00001111, 0b11111111],
        DecodeSliceError::Unaligned,
    );
}

#[test]
fn decode_slice_corrupted() {
    decode_slice_with_error::<32>(
        &[0b00100100, 0b00000000, 0b00111111, 0b11111111],
        DecodeSliceError::Corrupted,
    );
}

#[test]
fn encode_slice_needs_more_space() {
    encode_slice_with_error::<5>(&[0u8; 512], EncodeSliceError::NeedsMoreSpace);
}

#[test]
fn encode_slice_empty() {
    encode_slice_round_trip(&[], &[0b00000000, 0b00001111, 0b11111111]);
}

#[test]
fn encode_slice_simple() {
    encode_slice_round_trip(
        &[0b00000100],
        &[0b01101110, 0b00000000, 0b00011111, 0b11111110],
    );
}

#[test]
fn encode_slice_alternating() {
    encode_slice_round_trip(
        &[0b01010101],
        &[0b10110110, 0b11010000, 0b00000000, 0b11111111, 0b11110000],
    );
}

#[test]
fn encode_slice_run_of_ones() {
    encode_slice_round_trip(
        &[0b11111111],
        &[
            0b00000000, 0b00001111, 0b11111110, 0b00000001, 0b00000000, 0b00001111, 0b11111111,
        ],
    );
}

#[test]
fn encode_slice_start_with_ones() {
    encode_slice_round_trip(
        &[0b10101010],
        &[
            0b00000000, 0b00001111, 0b11111110, 0b11011011, 0b01100000, 0b00000000, 0b11111111,
            0b11110000,
        ],
    );
}

#[test]
fn encode_slice_short_run_of_zeroes() {
    encode_slice_round_trip(
        &[0x00; 540],
        &[
            0b00000000, 0b00010000, 0b11100001, 0b00000000, 0b00001111, 0b11111111,
        ],
    );
}

#[test]
fn encode_slice_long_run_of_zeroes() {
    encode_slice_round_trip(
        &[0x00; 2048],
        &[
            0b00000000, 0b00001111, 0b11111101, 0b00000000, 0b00010000, 0b00000101, 0b00000000,
            0b00001111, 0b11111111,
        ],
    );
}

#[test]
fn decode_slice_long_run_of_zeroes() {
    decode_slice_round_trip(
        &[
            0b00000000, 0b00010000, 0b11100001, 0b00000000, 0b00001111, 0b11111111,
        ],
        &[0x00; 540],
    );
}

#[test]
fn decode_slice_continuation_zeroes() {
    decode_slice_round_trip(
        &[
            0b00000000, 0b00001111, 0b11111101, 0b01010000, 0b00000000, 0b11111111, 0b11110000,
        ],
        &[0b00000000; 1536],
    );
}

#[test]
fn decode_slice_continuation_ones() {
    let mut output = [0u8; 516];
    output[2..516].fill(0xFF);

    decode_slice_round_trip(
        &[
            0b00010001, 0b00000000, 0b00001111, 0b11111101, 0b00000100, 0b00000000, 0b00111111,
            0b11111100,
        ],
        &output,
    );
}

#[test]
fn decode_slice_continuation_zeroes_special() {
    let mut output = [0u8; 1536];
    output[1535] = 0b00001111;

    decode_slice_round_trip(
        &[
            0b00000000, 0b00001111, 0b11111101, 0b00000000, 0b00001111, 0b11111110, 0b00010000,
            0b00000000, 0b11111111, 0b11110000,
        ],
        &output,
    );
}

#[test]
fn decode_slice_continuation_ones_special() {
    let mut output = [0u8; 516];
    output[2..515].fill(0xFF);
    output[515] = 0b11000000;

    decode_slice_round_trip(
        &[
            0b00010001, 0b00000000, 0b00001111, 0b11111101, 0b00000000, 0b00001111, 0b11111110,
            0b01110000, 0b00000000, 0b11111111, 0b11110000,
        ],
        &output,
    );
}

#[test]
fn decode_continuation_via_mode_change() {
    decode_slice_alternative(
        &[
            0b00111100, 0b00000000, 0b00111111, 0b11111000, 0b01001010, 0b00000000, 0b00011111,
            0b11111110,
        ],
        &[0b00000000, 0b00000000, 0b00000000, 0b00000001],
        &[0b00001000, 0b00100000, 0b00000001, 0b11111111, 0b11100000],
    );
}

#[test]
fn decode_corrupted() {
    let mut decoder = Decoder::new();

    let mut buffer = [0u8; 32];

    let (consumed_len, produced_len, state) = decoder.step(
        &[0b10001010, 0b10000000, 0b00000111, 0b11111111, 0b10110110],
        &mut buffer,
    );

    assert_eq!(consumed_len, 5);
    assert_eq!(produced_len, 1);
    assert_eq!(
        state,
        DecoderState::Terminated {
            corrupted: true,
            unaligned: false
        }
    );
    assert_eq!(decoder.partial_output_byte(), None);
    assert_eq!(&buffer[..produced_len], &[0b01110000]);
}

#[test]
fn decode_corrupted_long() {
    let mut decoder = Decoder::new();

    let mut buffer = [0u8; 32];

    let (consumed_len, produced_len, state) = decoder.step(
        &[0b00111100, 0b00101100, 0b00000000, 0b00011111, 0b11111111],
        &mut buffer,
    );

    assert_eq!(consumed_len, 5);
    assert_eq!(produced_len, 3);
    assert_eq!(
        state,
        DecoderState::Terminated {
            corrupted: true,
            unaligned: false
        }
    );
    assert_eq!(decoder.partial_output_byte(), None);
    assert_eq!(
        &buffer[..produced_len],
        &[0b00000000, 0b00000011, 0b11100000]
    );
}

#[test]
fn decode_unaligned() {
    let mut decoder = Decoder::new();

    let mut buffer = [0u8; 32];

    let (consumed_len, produced_len, state) = decoder.step(
        &[0b10001000, 0b00000000, 0b01111111, 0b11111000],
        &mut buffer,
    );

    assert_eq!(consumed_len, 4);
    assert_eq!(produced_len, 0);
    assert_eq!(
        state,
        DecoderState::Terminated {
            corrupted: false,
            unaligned: true
        }
    );
    assert_eq!(decoder.partial_output_byte(), Some((0b0111, 4)));
}

#[test]
fn decode_unaligned_long() {
    let mut decoder = Decoder::new();

    let mut buffer = [0u8; 32];

    let (consumed_len, produced_len, state) = decoder.step(
        &[0b00011101, 0b10000000, 0b00000111, 0b11111111, 0b10000000],
        &mut buffer,
    );

    assert_eq!(consumed_len, 5);
    assert_eq!(produced_len, 3);
    assert_eq!(
        state,
        DecoderState::Terminated {
            corrupted: false,
            unaligned: true
        }
    );
    assert_eq!(decoder.partial_output_byte(), Some((0b00001, 5)));
    assert_eq!(
        &buffer[..produced_len],
        &[0b00000000, 0b00000000, 0b00000000]
    );
}

#[test]
fn decode_unaligned_corrupted() {
    let mut decoder = Decoder::new();

    let mut buffer = [0u8; 32];

    let (consumed_len, produced_len, state) = decoder.step(
        &[0b10001000, 0b00000000, 0b01111111, 0b11111011],
        &mut buffer,
    );

    assert_eq!(consumed_len, 4);
    assert_eq!(produced_len, 0);
    assert_eq!(
        state,
        DecoderState::Terminated {
            corrupted: true,
            unaligned: true
        }
    );
    assert_eq!(decoder.partial_output_byte(), Some((0b0111, 4)));
}

#[test]
fn decode_unaligned_corrupted_long() {
    let mut decoder = Decoder::new();

    let mut buffer = [0u8; 32];

    let (consumed_len, produced_len, state) = decoder.step(
        &[0b00011101, 0b10000000, 0b00000111, 0b11111111, 0b10010110],
        &mut buffer,
    );

    assert_eq!(consumed_len, 5);
    assert_eq!(produced_len, 3);
    assert_eq!(
        state,
        DecoderState::Terminated {
            corrupted: true,
            unaligned: true
        }
    );
    assert_eq!(decoder.partial_output_byte(), Some((0b00001, 5)));
    assert_eq!(
        &buffer[..produced_len],
        &[0b00000000, 0b00000000, 0b00000000]
    );
}

#[test]
fn decode_streaming() {
    let mut buffer = [0u8; 32];

    let mut decoder = Decoder::new();

    assert_eq!(decoder.partial_output_byte(), None);

    assert_eq!(
        decoder.step(&[], &mut buffer[0..]),
        (0, 0, DecoderState::CanConsume)
    );

    assert_eq!(
        decoder.step(&[0b00111100], &mut buffer[0..]),
        (1, 0, DecoderState::CanConsume)
    );

    assert_eq!(
        decoder.step(&[0b00010010], &mut buffer[0..]),
        (1, 0, DecoderState::CanConsume)
    );

    assert_eq!(
        decoder.step(&[0b00000000], &mut buffer[0..]),
        (1, 1, DecoderState::CanConsume)
    );

    assert_eq!(&buffer[..1], &[0b00000000]);

    assert_eq!(decoder.partial_output_byte(), None);

    assert_eq!(
        decoder.step(&[0b00000011], &mut buffer[1..]),
        (1, 1, DecoderState::CanConsume)
    );

    assert_eq!(&buffer[..2], &[0b00000000, 0b00000011]);

    assert_eq!(
        decoder.step(&[], &mut buffer[2..]),
        (0, 0, DecoderState::CanConsume)
    );

    assert_eq!(
        decoder.step(&[0b11111111], &mut buffer[2..2]),
        (1, 0, DecoderState::CanProduce)
    );

    assert_eq!(decoder.partial_output_byte(), None);

    assert_eq!(
        decoder.step(&[0b11000000], &mut buffer[2..]),
        (
            1,
            1,
            DecoderState::Terminated {
                corrupted: false,
                unaligned: true
            }
        )
    );

    assert_eq!(&buffer[..3], &[0b00000000, 0b00000011, 0b11110000]);
    assert_eq!(decoder.partial_output_byte(), Some((0b0000, 3)));

    assert_eq!(
        decoder.step(&[0b01010101], &mut buffer[3..]),
        (
            0,
            0,
            DecoderState::Terminated {
                corrupted: false,
                unaligned: true
            }
        )
    );
}

#[test]
fn encode_streaming() {
    let mut buffer = [0u8; 32];

    let mut encoder = Encoder::new();

    assert_eq!(
        encoder.step(&[], &mut buffer[0..]),
        (0, 0, EncoderState::CanConsume)
    );

    assert_eq!(
        encoder.step(&[0b11001100], &mut buffer[0..]),
        (1, 3, EncoderState::CanConsume)
    );

    assert_eq!(&buffer[..3], &[0b00000000, 0b00001111, 0b11111110]);

    assert_eq!(
        encoder.step(&[], &mut buffer[3..]),
        (0, 0, EncoderState::CanConsume)
    );

    assert_eq!(
        encoder.step(&[0b11111111], &mut buffer[3..3]),
        (1, 0, EncoderState::CanProduce)
    );

    assert_eq!(
        encoder.step(&[], &mut buffer[3..3]),
        (0, 0, EncoderState::CanProduce)
    );

    assert_eq!(
        encoder.step(&[], &mut buffer[3..]),
        (0, 0, EncoderState::CanConsume)
    );

    assert_eq!(
        encoder.step(&[0b00000001], &mut buffer[3..]),
        (1, 2, EncoderState::CanConsume)
    );

    assert_eq!(
        &buffer[..5],
        &[0b00000000, 0b00001111, 0b11111110, 0b01110111, 0b00000001]
    );

    assert_eq!(
        encoder.step(&[], &mut buffer[5..]),
        (0, 0, EncoderState::CanConsume)
    );

    encoder.set_consumed_bytes_end();

    assert_eq!(
        encoder.step(&[], &mut buffer[5..]),
        (0, 4, EncoderState::Terminated)
    );

    assert_eq!(
        &buffer[..9],
        &[
            0b00000000, 0b00001111, 0b11111110, 0b01110111, 0b00000001, 0b00100010, 0b00000000,
            0b00011111, 0b11111110
        ]
    );

    assert_eq!(
        encoder.step(&[0b01010101], &mut buffer[9..]),
        (0, 0, EncoderState::Terminated)
    );
}
