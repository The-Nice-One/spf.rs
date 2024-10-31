pub(crate) use super::byte;
pub(crate) use super::MAGIC_BYTES;
pub(crate) use std::collections::HashMap;

/// specifies the .spf file format version
#[derive(Debug)]
pub enum FormatVersion {
    FV0000,
}
/// Specifies the default alignment for all characters in a font
#[derive(PartialEq, Debug)]
pub enum Alignment {
    Height,
    Width,
}
/// Represents a bitmap for a character in your font.
/// Note: This is a one dimensional vector, you can use the `get_pixel()` method to get a two dimensional-like interface.
/// Note: Only the first `width * height` items are used, the rest are ignored when encoding and decoding from/to a `Vec<u8>`
#[derive(Debug, Clone)]
pub struct Bitmap {
    pub width: u8,
    pub height: u8,
    pub data: Vec<bool>,
    inferred: bool,
}

impl Bitmap {
    pub fn new(width: u8, height: u8, data: Vec<bool>) -> Self {
        Self {
            width: width,
            height: height,
            data: data,
            inferred: false,
        }
    }
    /// Creates an inferred `Bitmap` struct which dimensions are unknown.
    ///
    /// This function is provided to make creating bitmaps for character much easier.
    /// Rather then providing the width and height, this Bitmap will automatically choose
    /// the right dimensions for the character bitmap depending on the `SimplePixelFont`
    /// struct `alignment`, and `size` fields. As such it is advised to use only inferred
    /// `Bitmap`'s when you use the `unchecked_add_character` or `add_character` methods of
    /// a `SimplePixelFont`
    ///
    /// # Example
    /// ```
    /// let font = SimplePixelFont::new(FormatVersion::FV0000, Alignment::Height, 4);
    /// font.unchecked_add_character("o", Bitmap::inferred(&[
    ///     false, true, true, false,
    ///     true, false, false, true,
    ///     true, false, false, true,
    ///     false, true, true, false
    /// ]));
    /// ```
    pub fn inferred(data: &[bool]) -> Self {
        Self {
            width: 0,
            height: 0,
            data: data.to_owned(),
            inferred: true,
        }
    }
    /// Returns a boolean depending if the Bitmap is inferred or not.
    ///
    /// Inferred Bitmap's can only be used when creating inferred characters.
    pub fn is_inferred(&self) -> bool {
        return self.inferred;
    }
    pub(crate) fn segment_into_u8s(&self) -> Vec<u8> {
        let mut chunks = self.data.chunks(8);
        let mut buffer: Vec<u8> = Vec::new();

        let mut iter = chunks.next();
        while !iter.is_none() {
            let chunk = iter.unwrap();
            let remainder = 8 - chunk.len();
            let mut byte = byte::Byte { bits: [false; 8] };
            let mut index: usize = 0;
            for pixel in chunk {
                byte.bits[index] = *pixel;
                index += 1;
            }
            for index in 8 - remainder..8 {
                byte.bits[index] = false;
            }
            buffer.push(byte.to_u8());
            iter = chunks.next();
        }
        return buffer;
    }
}
/// Represents a charater in the font.
#[derive(Clone, Debug)]
pub struct Character {
    pub utf8: char,
    pub size: u8,
    pub bitmap: Bitmap,
}

impl Character {
    pub fn new(utf8: char, size: u8, bitmap: Bitmap) -> Self {
        Self {
            utf8: utf8,
            size: size,
            bitmap: bitmap,
        }
    }
    pub fn inferred(utf8: char, bitmap: Bitmap) -> Self {
        if bitmap.is_inferred() {
            return Self {
                utf8: utf8,
                size: 0,
                bitmap: bitmap,
            };
        }
        panic!("Not an inferred bitmap.")
    }
}
/// Main structure that supports encoding and decoding with its defined methods.
#[derive(Debug)]
pub struct SimplePixelFont {
    pub version: FormatVersion,
    pub alignment: Alignment,
    pub size: u8,
    pub characters: Vec<Character>,

    #[cfg(feature = "cache")]
    pub cache: HashMap<char, usize>,
}

impl SimplePixelFont {
    pub fn new(format_version: FormatVersion, alignment: Alignment, size: u8) -> Self {
        #[cfg(feature = "cache")]
        return Self {
            version: format_version,
            alignment: alignment,
            size: size,
            characters: Vec::new(),
            cache: HashMap::new(),
        };

        #[cfg(not(feature = "cache"))]
        Self {
            version: format_version,
            alignment: alignment,
            size: size,
            characters: characters,
        }
    }
    pub fn add_character(&mut self, character: Character) {
        if character.bitmap.is_inferred() {
            if self.alignment == Alignment::Height {
                let width = (character.bitmap.data.len() as u16 / self.size as u16) as u8;
                self.characters.push(Character::new(
                    character.utf8,
                    width,
                    Bitmap::new(width, self.size, character.bitmap.data),
                ));
            }
        }
    }
    /// Encodes the structure into a `Vec<u8>` that can then be written to a file using `std::fs`
    pub fn to_vec_u8(&self) -> Vec<u8> {
        let mut buffer: Vec<u8> = Vec::new();
        buffer.push(102);
        buffer.push(115);
        buffer.push(70);

        buffer.push(
            byte::Byte {
                bits: [
                    self.alignment == Alignment::Width,
                    false,
                    false,
                    false,
                    false,
                    false,
                    false,
                    false,
                ],
            }
            .to_u8(),
        );

        buffer.push(self.size);

        for character in &self.characters {
            let mut char_buffer = [0; 4];
            character.utf8.encode_utf8(&mut char_buffer);
            for byte in char_buffer {
                if byte != 0 {
                    buffer.push(byte);
                }
            }

            buffer.push(character.size);
            buffer.append(&mut character.bitmap.segment_into_u8s());
        }

        let checksum = byte::three_byte_checksum(&buffer);
        buffer.insert(5, checksum[0]);
        buffer.insert(6, checksum[1]);
        buffer.insert(7, checksum[2]);
        return buffer;
    }

    /// Decodes a `Vec<u8>` and parses it into a struct, this method will check and make sure checksums are correct.
    pub fn from_vec_u8(buffer: Vec<u8>) -> Option<Self> {
        let mut local_buffer = buffer.clone();
        let mut file_checksum: [u8; 3] = [0, 0, 0];
        file_checksum[0] = local_buffer.remove(5);
        file_checksum[1] = local_buffer.remove(5);
        file_checksum[2] = local_buffer.remove(5);

        let checksum = byte::three_byte_checksum(&local_buffer);
        if !(file_checksum == checksum) {
            return None;
        }
        return Some(SimplePixelFont::unchecked_from_vec_u8(buffer));
    }

    /// Decodes a `Vec<u8>` and parses it into a struct, this method will ignore the checksum values.
    pub fn unchecked_from_vec_u8(buffer: Vec<u8>) -> Self {
        let mut buffer = buffer.clone();
        buffer.remove(5);
        buffer.remove(5);
        buffer.remove(5);
        let mut current_index = 0;
        let mut chunks = buffer.iter();

        let mut format_version: FormatVersion = FormatVersion::FV0000;
        let mut alignment: Alignment = Alignment::Height;
        let mut size: u8 = 0;
        let mut characters: Vec<Character> = Vec::new();
        let mut character_definition_stage = 0;
        let mut current_character: Character = Character {
            utf8: ' ',
            size: 0,
            bitmap: Bitmap {
                width: 0,
                height: 0,
                data: vec![],
                inferred: false,
            },
        };

        #[cfg(feature = "cache")]
        let mut cache: HashMap<char, usize> = HashMap::new();

        let mut iter = chunks.next();
        while !iter.is_none() {
            let chunk = iter.unwrap();
            if current_index < 3 {
                if !chunk == MAGIC_BYTES[current_index] {
                    panic!("File is not signed")
                }
            } else if current_index == 3 {
                if byte::Byte::from_u8(chunk.clone()).bits[0] == false {
                    alignment = Alignment::Height;
                } else {
                    alignment = Alignment::Width;
                }
                if byte::Byte::from_u8(chunk.clone()).bits[3..] == [false, false, false, false] {
                    format_version = FormatVersion::FV0000;
                }
            } else if current_index == 4 {
                size = chunk.clone();
            } else {
                if character_definition_stage == 0 {
                    let utf81 = byte::Byte::from_u8(chunk.clone());
                    let mut utf8_bytes: [u8; 4] = [0, 0, 0, 0];

                    if utf81.bits[7..] == [false] {
                        utf8_bytes[0] = utf81.to_u8();
                    } else if utf81.bits[5..] == [false, true, true, true] {
                        utf8_bytes[0] = utf81.to_u8();
                        iter = chunks.next();
                        utf8_bytes[1] = iter.unwrap().clone();
                    } else if utf81.bits[4..] == [false, true, true, true] {
                        utf8_bytes[0] = utf81.to_u8();
                        iter = chunks.next();
                        utf8_bytes[1] = iter.unwrap().clone();
                        iter = chunks.next();
                        utf8_bytes[2] = iter.unwrap().clone();
                    } else if utf81.bits[3..] == [false, true, true, true, true] {
                        utf8_bytes[0] = utf81.to_u8();
                        iter = chunks.next();
                        utf8_bytes[1] = iter.unwrap().clone();
                        iter = chunks.next();
                        utf8_bytes[2] = iter.unwrap().clone();
                        iter = chunks.next();
                        utf8_bytes[3] = iter.unwrap().clone();
                    }

                    current_character.utf8 = String::from_utf8(utf8_bytes.to_vec())
                        .unwrap()
                        .chars()
                        .next()
                        .unwrap();
                    character_definition_stage += 1;
                } else if character_definition_stage == 1 {
                    current_character.size = chunk.clone();
                    character_definition_stage += 1
                } else if character_definition_stage == 2 {
                    if alignment == Alignment::Height {
                        current_character.bitmap.height = size;
                        current_character.bitmap.width = current_character.size;
                    } else {
                        current_character.bitmap.height = current_character.size;
                        current_character.bitmap.width = size;
                    }

                    let bytes_used = (((current_character.bitmap.height as f32
                        * current_character.bitmap.width as f32)
                        as f32
                        / 8.0) as f32)
                        .ceil() as u8;

                    let remainder = bytes_used as i32 * 8 as i32
                        - (current_character.bitmap.height * current_character.bitmap.width) as i32;

                    let mut current_byte = byte::Byte::from_u8(iter.unwrap().clone());
                    for i in 0..bytes_used {
                        let mut counter = 0;
                        for bit in current_byte.bits {
                            if !(i == bytes_used - 1 && counter >= 8 - remainder) {
                                current_character.bitmap.data.push(bit);
                            }
                            counter += 1;
                        }

                        if i != bytes_used - 1 {
                            iter = chunks.next();
                            current_byte = byte::Byte::from_u8(iter.unwrap().clone());
                        }
                    }

                    #[cfg(feature = "cache")]
                    cache.insert(current_character.utf8, characters.len());

                    characters.push(current_character.clone());
                    current_character.bitmap.data = vec![];
                    character_definition_stage = 0;
                }
            }
            iter = chunks.next();
            current_index += 1;
        }

        #[cfg(feature = "cache")]
        return Self {
            version: format_version,
            alignment: alignment,
            size: size,
            characters: characters,
            cache: cache,
        };

        #[cfg(not(feature = "cache"))]
        Self {
            version: format_version,
            alignment: alignment,
            size: size,
            characters: characters,
        }
    }
}
