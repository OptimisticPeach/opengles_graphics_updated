//! Glyph caching

use {rusttype, graphics};
use crate::{Texture, TextureSettings};
use std::collections::HashMap;
use graphics::types::Scalar;

extern crate fnv;
use self::fnv::FnvHasher;
use std::hash::BuildHasherDefault;

use std::path::Path;
use std::io::Read;
use std::fs::File;
use crate::error::Error;

pub use graphics::types::FontSize;
use graphics::character::CharacterCache;

/// The type alias for font characters.
pub type Character<'a> = graphics::character::Character<'a, Texture>;

/// A struct used for caching rendered font.
pub struct GlyphCache<'a> {
    /// The font.
    pub font: rusttype::Font<'a>,
    /// The settings to render the font with.
    settings: TextureSettings,
    // Maps from fontsize and character to offset, size and texture.
    data: HashMap<(FontSize, char),
                  ([Scalar; 2], [Scalar; 2], Texture),
                  BuildHasherDefault<FnvHasher>>,
}

impl<'a> GlyphCache<'a> {
    /// Constructs a GlyphCache from a Font.
    pub fn from_font(font: rusttype::Font<'a>, settings: TextureSettings) -> Self {
        let fnv = BuildHasherDefault::<FnvHasher>::default();
        GlyphCache {
            font: font,
            settings: settings,
            data: HashMap::with_hasher(fnv),
        }
    }

    /// Constructor for a GlyphCache.
    pub fn new<P>(font: P, settings: TextureSettings) -> Result<GlyphCache<'static>, Error>
        where P: AsRef<Path>
    {
        let fnv = BuildHasherDefault::<FnvHasher>::default();
        let mut file = File::open(font)?;
        let mut file_buffer = Vec::new();
        file.read_to_end(&mut file_buffer)?;

        let collection = rusttype::FontCollection::from_bytes(file_buffer).unwrap();
        let font = collection.into_font().unwrap();
        Ok(GlyphCache {
            font: font,
            settings: settings,
            data: HashMap::with_hasher(fnv),
        })
    }

    /// Creates a GlyphCache for a font stored in memory.
    pub fn from_bytes(font: &'a [u8], settings: TextureSettings) -> Result<GlyphCache<'a>, Error> {
        let collection = rusttype::FontCollection::from_bytes(font).unwrap();
        let font = collection.into_font().unwrap();
        Ok(Self::from_font(font, settings))
    }

    /// Load all characters in the `chars` iterator for `size`
    pub fn preload_chars<I>(&mut self, size: FontSize, chars: I)
        where I: Iterator<Item = char>
    {
        for ch in chars {
            self.character(size, ch);
        }
    }

    /// Load all the printable ASCII characters for `size`. Includes space.
    pub fn preload_printable_ascii(&mut self, size: FontSize) {
        // [0x20, 0x7F) contains all printable ASCII characters ([' ', '~'])
        self.preload_chars(size, (0x20u8..0x7F).map(|ch| ch as char));
    }

    /// Return `ch` for `size` if it's already cached. Don't load.
    /// See the `preload_*` functions.
    pub fn opt_character(&self, size: FontSize, ch: char) -> Option<Character> {
        self.data.get(&(size, ch)).map(|&(offset, size, ref texture)| {
            Character {
                offset: offset,
                size: size,
                texture: texture,
            }
        })
    }
}

impl<'b> CharacterCache for GlyphCache<'b> {
    type Texture = Texture;
    type Error = Error;

    fn character<'a>(&'a mut self, size: FontSize, ch: char) -> Result<Character<'a>, Error> {
        use std::collections::hash_map::Entry;
        use rusttype as rt;

        let size = ((size as f32) * 1.333).round() as u32; // convert points to pixels

        match self.data.entry((size, ch)) {
            //returning `into_mut()' to get reference with 'a lifetime
            Entry::Occupied(v) => {
                let &mut (offset, size, ref texture) = v.into_mut();
                Ok(
                    Character {
                        offset: offset,
                        size: size,
                        texture: texture,
                    }
                )
            }
            Entry::Vacant(v) => {
                // this is only None for invalid GlyphIds,
                // but char is converted to a Codepoint which must result in a glyph.
                let glyph = self.font.glyph(ch);
                let scale = rt::Scale::uniform(size as f32);
                let mut glyph = glyph.scaled(scale);

                // some fonts do not contain glyph zero as fallback, instead try U+FFFD.
                if glyph.id() == rt::GlyphId(0) && glyph.shape().is_none() {
                    glyph = self.font.glyph('\u{FFFD}').scaled(scale);
                }

                let h_metrics = glyph.h_metrics();
                let bounding_box = glyph.exact_bounding_box().unwrap_or(rt::Rect {
                    min: rt::Point { x: 0.0, y: 0.0 },
                    max: rt::Point { x: 0.0, y: 0.0 },
                });
                let glyph = glyph.positioned(rt::point(0.0, 0.0));
                let pixel_bounding_box = glyph.pixel_bounding_box().unwrap_or(rt::Rect {
                    min: rt::Point { x: 0, y: 0 },
                    max: rt::Point { x: 0, y: 0 },
                });
                let pixel_bb_width = pixel_bounding_box.width() + 2;
                let pixel_bb_height = pixel_bounding_box.height() + 2;

                let mut image_buffer = Vec::<u8>::new();
                image_buffer.resize((pixel_bb_width * pixel_bb_height) as usize, 0);
                glyph.draw(|x, y, v| {
                    let pos = ((x + 1) + (y + 1) * (pixel_bb_width as u32)) as usize;
                    image_buffer[pos] = (255.0 * v) as u8;
                });

                let &mut (offset, size, ref texture) =
                    v.insert(([bounding_box.min.x as Scalar - 1.0,
                               -pixel_bounding_box.min.y as Scalar + 1.0],
                              [h_metrics.advance_width as Scalar, 0 as Scalar],
                              {
                                  if pixel_bb_width == 0 || pixel_bb_height == 0 {
                                      Texture::empty().unwrap()
                                  } else {
                                      Texture::from_memory_alpha(&image_buffer,
                                                                 pixel_bb_width as u32,
                                                                 pixel_bb_height as u32,
                                                                 &self.settings)
                                          .unwrap()
                                  }
                              }));
                Ok(Character {
                    offset: offset,
                    size: size,
                    texture: texture,
                })
            }
        }
    }
}
