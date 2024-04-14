# zerocrush

[![Documentation](https://docs.rs/zerocrush/badge.svg)](https://docs.rs/zerocrush)
[![Crates.io](https://img.shields.io/crates/v/zerocrush.svg)](https://crates.io/crates/zerocrush)

This repository implements a compression algorithm in safe Rust designed for data which mostly consists of long runs of zero bits, with the characteristic that it requires very little memory and code size to compress and decompress while attaining very satisfactory compression ratios on such inputs.

**This is not intended as an interchange format.**

## Rationale

The original motivation for developing this library was to compress FPGA bitstreams for inclusion in embedded software on resource-constrained devices. We wish to compress the bitstream (which tends to be quite sparse) as much as possible without resorting to resource-intensive decompressors such as zlib whose memory requirements and code size make them unsuitable for this purpose. In contrast, the zerocrush decompressor requires only 20 bytes of state on a 32-bit target and has a very small code footprint while still achieving reasonable throughput.

The prefix code for zerocrush is inspired by previous work done by the IceStorm project in the form of [icecompr](https://github.com/YosysHQ/icestorm/tree/master/icecompr) and pushes the approach slightly further to attain measurably better compression ratios on typical bitstreams in addition to streamlining the resulting compressor and decompressor implementations to be reasonably efficient on embedded devices.

## Prefix Code Symbols

The compressed representation consists of a concatenation of prefix code symbols, MSB first, zero-padded to a multiple of eight bits. The symbols are taken from two different prefix codes referred to as "mode 0" and "mode 1", alternating on every symbol (except in the case of a special symbol as described further below) beginning with mode 0.

Mode 0 table:

| Symbol                     | Represents            |
| -------------------------- | --------------------- |
| `1x`                       | `0` × [1 to 2]        |
| `01xx`                     | `0` × [3 to 6]        |
| `001xxx`                   | `0` × [7 to 14]       |
| `0001xxxx`                 | `0` × [15 to 30]      |
| `00001xxxxx`               | `0` × [31 to 62]      |
| `000001xxxxxx`             | `0` × [63 to 126]     |
| `0000001xxxxxxx`           | `0` × [127 to 254]    |
| `00000001xxxxxxxx`         | `0` × [255 to 510]    |
| `000000001xxxxxxxxx`       | `0` × [511 to 1022]   |
| `0000000001xxxxxxxxxx`     | `0` × [1023 to 2046]  |
| `00000000001xxxxxxxxxxx`   | `0` × [2047 to 4094]  |
| `000000000001xxxxxxxxxxxx` | `0` × [4095 to 8190]  |
| `000000000000xxxxxxxxxxxx` | `0` × [8191 to 12284] |

Mode 1 table:

| Symbol                     | Represents            |
| -------------------------- | --------------------- |
| `1`                        | `1` × 1               |
| `01`                       | `1` × 2               |
| `001`                      | `1` × 3               |
| `0001`                     | `1` × 4               |
| `00001`                    | `1` × 5               |
| `000001`                   | `1` × 6               |
| `0000001`                  | `1` × 7               |
| `00000001`                 | `1` × 8               |
| `000000001`                | `1` × 9               |
| `0000000001`               | `1` × 10              |
| `00000000001`              | `1` × 11              |
| `000000000001`             | `1` × 12              |
| `000000000000xxxxxxxxxxxx` | `1` × [13 to 4106]    |

The following three symbols have the same representation in both modes and carry special meaning affecting the decompressor's operation. The mode change and termination symbols do not represent any data while the continuated symbol represents data according to the mode tables above.

| Symbol                     | Description        |
| -------------------------- | ------------------ |
| `000000000000111111111101` | Continuated symbol |
| `000000000000111111111110` | Mode change symbol |
| `000000000000111111111111` | Termination symbol |

The continuated symbol is designed to help represent arbitrarily long runs of zeroes and ones and causes the decompressor to not change mode. The mode change symbol causes the decompressor to change mode immediately and is normally only encountered either at the start of the compressed stream if the input happens to not start with a zero bit, or following the continuated symbol if the continuated data would have length zero. The termination symbol is assumed to be the last symbol in the compressed stream.

No framing or checksumming mechanism is built into this representation. Any bit sequence ending in a termination symbol represents a valid compressed stream, however the library is capable of verifying that the compressed stream is correctly zero-padded and that the decompressed output ends on a byte boundary. Additional checks can be added at a higher level by e.g. prepending a header and appending a checksum to the compressed stream.
