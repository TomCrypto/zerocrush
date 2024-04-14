//! Low-overhead compression for sparse files.

#![no_std]
#![forbid(unsafe_code)]
#![forbid(missing_docs)]

/// Possible states the decoder can be in.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DecoderState {
    /// The decoder is still able to consume bytes.
    CanConsume,
    /// The decoder is still able to produce bytes.
    CanProduce,
    /// The decoder has reached the terminal state.
    Terminated {
        /// Whether the encoded data was not padded using zero bits.
        corrupted: bool,
        /// Whether the decoded data did not end on a byte boundary.
        unaligned: bool,
    },
}

/// Streaming decoder context.
#[derive(Debug)]
pub struct Decoder {
    symbol_bits: usize,
    queued_bits: usize,
    output_bits: usize,

    symbol_data: u32,
    output_data: u8,

    queued_mode: bool,
    symbol_mode: bool,
    symbol_term: bool,
}

impl Decoder {
    /// Constructs a new decoder instance in its initial state.
    pub const fn new() -> Self {
        Self {
            symbol_bits: 0,
            queued_bits: 0,
            output_bits: 0,

            symbol_data: 0,
            output_data: 0,

            queued_mode: false,
            symbol_mode: false,
            symbol_term: false,
        }
    }

    /// Resets this decoder instance to its initial state.
    pub fn reset(&mut self) {
        *self = Self::new();
    }

    /// Steps this decoder instance, returning a `(bytes consumed, bytes produced, state)` tuple.
    pub fn step(&mut self, consumed: &[u8], produced: &mut [u8]) -> (usize, usize, DecoderState) {
        let mut consumed_len = 0;
        let mut produced_len = 0;

        loop {
            if self.consume(consumed, &mut consumed_len) {
                return (consumed_len, produced_len, DecoderState::CanConsume);
            }

            if self.produce(produced, &mut produced_len) {
                return (consumed_len, produced_len, DecoderState::CanProduce);
            }

            if self.symbol_term {
                break;
            }
        }

        debug_assert!(self.symbol_bits <= 7);
        debug_assert!(self.queued_bits == 0);

        (
            consumed_len,
            produced_len,
            DecoderState::Terminated {
                corrupted: self.symbol_data != 0,
                unaligned: self.output_bits != 0,
            },
        )
    }

    /// Retrieves the (right-aligned) last partial output byte.
    pub fn partial_output_byte(&self) -> Option<(u8, usize)> {
        if self.symbol_term && self.output_bits != 0 {
            Some((self.output_data, self.output_bits))
        } else {
            None
        }
    }

    fn consume(&mut self, consumed: &[u8], consumed_len: &mut usize) -> bool {
        if self.queued_bits == 0 && !self.symbol_term {
            while self.symbol_bits < 24 {
                let Some(&input_byte) = consumed.get(*consumed_len) else {
                    return true;
                };

                self.symbol_data |= (input_byte as u32) << (24 - self.symbol_bits);
                self.symbol_bits += 8;
                *consumed_len += 1;
            }

            let shift_out;
            let mut count: usize = 0;
            let mut cont = false;

            let prefix_len = self.symbol_data.leading_zeros() as usize;

            if prefix_len < 12 {
                if self.symbol_mode {
                    shift_out = prefix_len + 1;
                    count = shift_out;
                } else {
                    shift_out = 2 * (prefix_len + 1);
                    let mask = (1 << (prefix_len + 1)) - 1;

                    let payload = (self.symbol_data >> (32 - shift_out)) as u16;

                    count = ((payload & mask) + mask) as usize;
                }
            } else {
                shift_out = 24;

                let payload = ((self.symbol_data >> 8) as u16) & 0b1111_1111_1111;

                if payload == 0xFFF {
                    self.symbol_term = true;
                } else if payload != 0xFFE {
                    if self.symbol_mode {
                        count = (payload + 13) as usize;
                    } else {
                        count = (payload + 8191) as usize;
                    }

                    if payload == 0xFFD {
                        cont = true;
                    }
                }
            }

            self.symbol_bits -= shift_out;
            self.symbol_data <<= shift_out;

            if count > 0 {
                self.queued_bits = count;
                self.queued_mode = self.symbol_mode;
            }

            if !cont {
                self.symbol_mode = !self.symbol_mode;
            }
        }

        false
    }

    fn produce(&mut self, produced: &mut [u8], produced_len: &mut usize) -> bool {
        if self.output_bits == 0 && self.queued_bits >= 8 {
            let transfer = (self.queued_bits / 8).min(produced.len() - *produced_len);

            if transfer == 0 {
                return true;
            }

            let slice = &mut produced[*produced_len..][..transfer];

            if self.queued_mode {
                slice.fill(0xFF);
            } else {
                slice.fill(0x00);
            }

            self.queued_bits -= transfer * 8;
            *produced_len += transfer;
        } else if self.output_bits != 8 && self.queued_bits != 0 {
            let amount = (8 - self.output_bits).min(self.queued_bits);

            let mut word = 0;

            if self.queued_mode {
                word = (1 << amount) - 1;
            }

            self.output_data <<= amount;
            self.output_data |= word;
            self.output_bits += amount;
            self.queued_bits -= amount;
        } else if self.output_bits == 8 {
            if let Some(byte) = produced.get_mut(*produced_len) {
                *byte = self.output_data;
                self.output_data = 0;
                self.output_bits = 0;
                *produced_len += 1;
            } else {
                return true;
            }
        }

        false
    }
}

impl Default for Decoder {
    fn default() -> Self {
        Self::new()
    }
}

/// Possible states the encoder can be in.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EncoderState {
    /// The encoder is still able to consume bytes.
    CanConsume,
    /// The encoder is still able to produce bytes.
    CanProduce,
    /// The encoder has reached the terminal state.
    Terminated,
}

/// Streaming encoder context.
#[derive(Debug)]
pub struct Encoder {
    symbol_bits: usize,
    queued_bits: usize,
    output_bits: usize,

    symbol_data: u32,
    output_data: u8,

    queued_done: bool,
    queued_mode: bool,
    symbol_term: bool,
    queued_term: bool,
    output_term: bool,
}

impl Encoder {
    /// Constructs a new encoder instance in its initial state.
    pub const fn new() -> Self {
        Self {
            symbol_bits: 0,
            queued_bits: 0,
            output_bits: 0,

            symbol_data: 0,
            output_data: 0,

            queued_done: false,
            queued_mode: false,
            symbol_term: false,
            queued_term: false,
            output_term: false,
        }
    }

    /// Resets this encoder instance to its initial state.
    pub fn reset(&mut self) {
        *self = Self::new();
    }

    /// Steps this encoder instance, returning a `(bytes consumed, bytes produced, state)` tuple.
    pub fn step(&mut self, consumed: &[u8], produced: &mut [u8]) -> (usize, usize, EncoderState) {
        let mut consumed_len = 0;
        let mut produced_len = 0;

        loop {
            if self.consume(consumed, &mut consumed_len) {
                return (consumed_len, produced_len, EncoderState::CanConsume);
            }

            if self.produce(produced, &mut produced_len) {
                return (consumed_len, produced_len, EncoderState::CanProduce);
            }

            if self.output_term {
                break;
            }
        }

        debug_assert!(self.symbol_bits == 0);
        debug_assert!(self.queued_bits == 0);
        debug_assert!(self.output_bits == 0);

        (consumed_len, produced_len, EncoderState::Terminated)
    }

    /// Informs the encoder that no further input bytes are available.
    pub fn set_consumed_bytes_end(&mut self) {
        self.queued_term = true;
    }

    fn consume(&mut self, consumed: &[u8], consumed_len: &mut usize) -> bool {
        if self.output_bits == 0 && !self.symbol_term {
            if let Some(&byte) = consumed.get(*consumed_len) {
                self.output_data = byte;
                self.output_bits = 8;
                *consumed_len += 1;
            } else if !self.queued_term {
                return true;
            }
        }

        if self.queued_done {
            return false;
        }

        if self.output_bits > 0 || self.queued_bits > 0 {
            let mut count = if self.queued_mode {
                self.output_data.leading_ones()
            } else {
                self.output_data.leading_zeros()
            } as usize;

            count = count.min(self.output_bits);

            if count == 0 {
                self.queued_mode = !self.queued_mode;
                self.queued_done = true;
            } else {
                if count < 8 {
                    self.output_data <<= count;
                }

                self.output_bits -= count;
                self.queued_bits += count;
            }
        } else {
            self.symbol_term = true;
        }

        false
    }

    fn produce(&mut self, produced: &mut [u8], produced_len: &mut usize) -> bool {
        if self.symbol_bits <= 8 {
            if self.symbol_term && !self.output_term {
                self.symbol_data |= 0b000000000000111111111111 << (8 - self.symbol_bits);

                if self.symbol_bits == 0 {
                    self.symbol_bits = 24;
                } else {
                    self.symbol_bits = 32;
                }
            } else if self.queued_done {
                let mut cont = false;

                if self.queued_mode {
                    match self.queued_bits {
                        0 => {
                            self.symbol_data |=
                                0b0000_0000_0000_1111_1111_1110 << (8 - self.symbol_bits);
                            self.symbol_bits += 24;
                        }
                        1..=2 => {
                            self.symbol_data |=
                                (0b10 | ((self.queued_bits - 1) as u32)) << (30 - self.symbol_bits);
                            self.symbol_bits += 2;
                        }
                        3..=6 => {
                            self.symbol_data |= (0b0100 | ((self.queued_bits - 3) as u32))
                                << (28 - self.symbol_bits);
                            self.symbol_bits += 4;
                        }
                        7..=14 => {
                            self.symbol_data |= (0b001000 | ((self.queued_bits - 7) as u32))
                                << (26 - self.symbol_bits);
                            self.symbol_bits += 6;
                        }
                        15..=30 => {
                            self.symbol_data |= (0b00010000 | ((self.queued_bits - 15) as u32))
                                << (24 - self.symbol_bits);
                            self.symbol_bits += 8;
                        }
                        31..=62 => {
                            self.symbol_data |= (0b0000100000 | ((self.queued_bits - 31) as u32))
                                << (22 - self.symbol_bits);
                            self.symbol_bits += 10;
                        }
                        63..=126 => {
                            self.symbol_data |= (0b000001000000 | ((self.queued_bits - 63) as u32))
                                << (20 - self.symbol_bits);
                            self.symbol_bits += 12;
                        }
                        127..=254 => {
                            self.symbol_data |= (0b00000010000000
                                | ((self.queued_bits - 127) as u32))
                                << (18 - self.symbol_bits);
                            self.symbol_bits += 14;
                        }
                        255..=510 => {
                            self.symbol_data |= (0b0000000100000000
                                | ((self.queued_bits - 255) as u32))
                                << (16 - self.symbol_bits);
                            self.symbol_bits += 16;
                        }
                        511..=1022 => {
                            self.symbol_data |= (0b000000001000000000
                                | ((self.queued_bits - 511) as u32))
                                << (14 - self.symbol_bits);
                            self.symbol_bits += 18;
                        }
                        1023..=2046 => {
                            self.symbol_data |= (0b00000000010000000000
                                | ((self.queued_bits - 1023) as u32))
                                << (12 - self.symbol_bits);
                            self.symbol_bits += 20;
                        }
                        2047..=4094 => {
                            self.symbol_data |= (0b0000000000100000000000
                                | ((self.queued_bits - 2047) as u32))
                                << (10 - self.symbol_bits);
                            self.symbol_bits += 22;
                        }
                        4095..=8190 => {
                            self.symbol_data |= (0b000000000001000000000000
                                | ((self.queued_bits - 4095) as u32))
                                << (8 - self.symbol_bits);
                            self.symbol_bits += 24;
                        }
                        8191..=12283 => {
                            self.symbol_data |=
                                ((self.queued_bits - 8191) as u32) << (8 - self.symbol_bits);
                            self.symbol_bits += 24;
                        }
                        12284.. => {
                            self.symbol_data |=
                                0b000000000000111111111101 << (8 - self.symbol_bits);
                            self.symbol_bits += 24;
                            self.queued_bits -= 12284;
                            cont = true;
                        }
                    }
                } else {
                    match self.queued_bits {
                        0 => {
                            self.symbol_data |=
                                0b0000_0000_0000_1111_1111_1110 << (8 - self.symbol_bits);
                            self.symbol_bits += 24;
                        }
                        1 => {
                            self.symbol_data |= 0b1 << (31 - self.symbol_bits);
                            self.symbol_bits += 1;
                        }
                        2 => {
                            self.symbol_data |= 0b01 << (30 - self.symbol_bits);
                            self.symbol_bits += 2;
                        }
                        3 => {
                            self.symbol_data |= 0b001 << (29 - self.symbol_bits);
                            self.symbol_bits += 3;
                        }
                        4 => {
                            self.symbol_data |= 0b0001 << (28 - self.symbol_bits);
                            self.symbol_bits += 4;
                        }
                        5 => {
                            self.symbol_data |= 0b00001 << (27 - self.symbol_bits);
                            self.symbol_bits += 5;
                        }
                        6 => {
                            self.symbol_data |= 0b000001 << (26 - self.symbol_bits);
                            self.symbol_bits += 6;
                        }
                        7 => {
                            self.symbol_data |= 0b0000001 << (25 - self.symbol_bits);
                            self.symbol_bits += 7;
                        }
                        8 => {
                            self.symbol_data |= 0b00000001 << (24 - self.symbol_bits);
                            self.symbol_bits += 8;
                        }
                        9 => {
                            self.symbol_data |= 0b000000001 << (23 - self.symbol_bits);
                            self.symbol_bits += 9;
                        }
                        10 => {
                            self.symbol_data |= 0b0000000001 << (22 - self.symbol_bits);
                            self.symbol_bits += 10;
                        }
                        11 => {
                            self.symbol_data |= 0b00000000001 << (21 - self.symbol_bits);
                            self.symbol_bits += 11;
                        }
                        12 => {
                            self.symbol_data |= 0b000000000001 << (20 - self.symbol_bits);
                            self.symbol_bits += 12;
                        }
                        13..=4105 => {
                            self.symbol_data |=
                                ((self.queued_bits - 13) as u32) << (8 - self.symbol_bits);
                            self.symbol_bits += 24;
                        }
                        4106.. => {
                            self.symbol_data |=
                                0b000000000000111111111101 << (8 - self.symbol_bits);
                            self.symbol_bits += 24;
                            self.queued_bits -= 4106;
                            cont = true;
                        }
                    }
                }

                if !cont {
                    self.queued_bits = 0;
                    self.queued_done = false;
                }
            }
        }

        while self.symbol_bits >= 8 {
            if let Some(byte) = produced.get_mut(*produced_len) {
                *byte = (self.symbol_data >> 24) as u8;

                self.symbol_data <<= 8;
                self.symbol_bits -= 8;
                *produced_len += 1;
            } else {
                return true;
            }
        }

        if self.symbol_term && self.symbol_bits == 0 {
            self.output_term = true;
        }

        false
    }
}

impl Default for Encoder {
    fn default() -> Self {
        Self::new()
    }
}

/// Errors that may occur while decoding a slice.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DecodeSliceError {
    /// More input bytes were required for decoding.
    TruncatedInput,
    /// More output space was required for decoding.
    NeedsMoreSpace,
    /// The encoded data was not padded using zero bits.
    Corrupted,
    /// The decoded data did not end on a byte boundary.
    Unaligned,
}

/// Convenient helper function to directly decode arbitrary data from a destination byte slice.
pub fn decode_from_slice(input: &[u8], output: &mut [u8]) -> Result<usize, DecodeSliceError> {
    let mut decoder = Decoder::new();

    let (_, produced_len, state) = decoder.step(input, output);

    match state {
        DecoderState::CanConsume => Err(DecodeSliceError::TruncatedInput),
        DecoderState::CanProduce => Err(DecodeSliceError::NeedsMoreSpace),
        DecoderState::Terminated {
            corrupted,
            unaligned,
        } => {
            if corrupted {
                Err(DecodeSliceError::Corrupted)
            } else if unaligned {
                Err(DecodeSliceError::Unaligned)
            } else {
                Ok(produced_len)
            }
        }
    }
}

/// Errors that may occur while encoding a slice.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EncodeSliceError {
    /// More output space was required for encoding.
    NeedsMoreSpace,
}

/// Convenient helper function to directly encode arbitrary data into a destination byte slice.
pub fn encode_into_slice(input: &[u8], output: &mut [u8]) -> Result<usize, EncodeSliceError> {
    let mut encoder = Encoder::new();
    encoder.set_consumed_bytes_end();

    let (_, produced_len, state) = encoder.step(input, output);

    match state {
        EncoderState::CanConsume => unreachable!("is given entire input"),
        EncoderState::CanProduce => Err(EncodeSliceError::NeedsMoreSpace),
        EncoderState::Terminated => Ok(produced_len),
    }
}

#[cfg(test)]
mod tests;
