//! # AtlasText — dynamic text atlas with skyline packer + direct swash rasterization
//!
//! Manages a multi-page sprite atlas for dynamic text rendering.
//! Rendering is done per frame: layout → rasterize → pack → upload.
//! No cosmic-text `SwashCache` is used; glyphs are rasterized directly via swash.

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::{Context, Result};
use cosmic_text::{Attrs, Buffer, FontSystem, Metrics, Shaping};
use glam::Vec2;
use swash::scale::{
    Render, ScaleContext, Source,
    image::{Content as SwashContent, Image as SwashImage},
};
use swash::zeno::{Format, Vector};
use windows::Win32::Graphics::{Direct3D11::*, Dxgi::Common::*};

use super::sprite2d::{HaveID, Sprite2D, Sprite2DBuffer, Sprite2DObject};
use super::transform2d::Transform2D;
use crate::graphic::d3d11::D3D11;
use crate::graphic::d3d11::d3d11_utils::{TextureInfo, create_srv, create_texture_2d};

// ─────────────────────────────────────────────────────────────────────
// Constants
// ─────────────────────────────────────────────────────────────────────

use crate::TextureInfoArced;

/// Atlas page width & height in pixels.
const PAGE_SIZE: u32 = 2048;
/// Packing margin (pixels) between glyphs to avoid bleeding.
const PACK_MARGIN: u32 = 1;

// ─────────────────────────────────────────────────────────────────────
// Skyline packer
// ─────────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug)]
struct Segment {
    x: u32,
    y: u32,
    width: u32,
}

struct SkylinePacker {
    skylines: Vec<Segment>,
    page_size: u32,
}

impl SkylinePacker {
    fn new(page_size: u32) -> Self {
        Self {
            skylines: vec![Segment {
                x: 0,
                y: 0,
                width: page_size,
            }],
            page_size,
        }
    }

    fn allocate(&mut self, w: u32, h: u32) -> Option<(u32, u32)> {
        if w == 0 || h == 0 || w > self.page_size || h > self.page_size {
            return None;
        }

        let mut best_idx = None;
        let mut best_y = u32::MAX;
        let mut best_x = u32::MAX;

        for (i, seg) in self.skylines.iter().enumerate() {
            if seg.width < w {
                continue;
            }
            let y = self.max_y(seg.x, seg.x + w);
            if y < best_y || (y == best_y && seg.x < best_x) {
                best_y = y;
                best_x = seg.x;
                best_idx = Some(i);
            }
        }

        let idx = best_idx?;
        let x = self.skylines[idx].x;
        let y = best_y;

        if y + h > self.page_size {
            return None;
        }

        let new_seg = Segment {
            x: x + w,
            y,
            width: w,
        };
        self.insert_segment(idx, new_seg);
        self.merge_neighbors();
        Some((x, y))
    }

    fn max_y(&self, x: u32, x_end: u32) -> u32 {
        let mut max_y = 0u32;
        for seg in &self.skylines {
            if seg.x >= x_end {
                break;
            }
            let seg_end = seg.x + seg.width;
            if seg_end > x && seg.y > max_y {
                max_y = seg.y;
            }
        }
        max_y
    }

    fn insert_segment(&mut self, idx: usize, new_seg: Segment) {
        let new_right = new_seg.x + new_seg.width;
        let insert_pos = idx;
        while insert_pos < self.skylines.len() && self.skylines[insert_pos].x < new_right {
            let seg = &mut self.skylines[insert_pos];
            let seg_right = seg.x + seg.width;
            if seg_right <= new_right {
                self.skylines.remove(insert_pos);
            } else {
                let trimmed_w = seg_right - new_right;
                seg.x = new_right;
                seg.width = trimmed_w;
                break;
            }
        }
        let mut pos = 0;
        while pos < self.skylines.len() && self.skylines[pos].x < new_seg.x {
            pos += 1;
        }
        self.skylines.insert(pos, new_seg);
    }

    fn merge_neighbors(&mut self) {
        let mut i = 0;
        while i + 1 < self.skylines.len() {
            let a = self.skylines[i];
            let b = self.skylines[i + 1];
            if a.y == b.y {
                self.skylines[i].width += b.width;
                self.skylines.remove(i + 1);
            } else {
                i += 1;
            }
        }
    }

    // Theorically, in no case this method should be called.
    // 理论上来讲，没有使用这个函数的情况
    //
    // fn clear(&mut self) {
    //     self.skylines.clear();
    //     self.skylines.push(Segment {
    //         x: 0,
    //         y: 0,
    //         width: self.page_size,
    //     });
    // }
}

// ─────────────────────────────────────────────────────────────────────
// TextPage
// ─────────────────────────────────────────────────────────────────────

/// A single 2048×2048 atlas page.
struct TextPage {
    texture: Arc<TextureInfo>,
    pixels: Vec<[u8; 4]>,
    packer: SkylinePacker,
    dirty: bool,
}

impl TextPage {
    fn new(device: &ID3D11Device) -> Result<Self> {
        let pw = PAGE_SIZE;
        let ph = PAGE_SIZE;

        let texture = create_texture_2d(
            device,
            pw,
            ph,
            DXGI_FORMAT_R8G8B8A8_UNORM,
            D3D11_BIND_SHADER_RESOURCE.0 as u32,
            D3D11_USAGE_DEFAULT,
            0,
            None,
        )?;
        let srv = create_srv(device, &texture, DXGI_FORMAT_R8G8B8A8_UNORM)?;

        Ok(Self {
            texture: Arc::new(TextureInfo {
                texture,
                srv,
                width: pw,
                height: ph,
                format: DXGI_FORMAT_R8G8B8A8_UNORM,
            }),
            pixels: vec![[0u8; 4]; (PAGE_SIZE * PAGE_SIZE) as usize],
            packer: SkylinePacker::new(PAGE_SIZE),
            dirty: false,
        })
    }

    /// Write glyph pixel data into the atlas.
    /// `image_data` is the raw bytes from SwashImage.
    /// `content` indicates the pixel format.
    fn allocate_and_write(
        &mut self,
        w: u32,
        h: u32,
        image_data: &[u8],
        content: SwashContent,
    ) -> Option<(u32, u32)> {
        let fw = w + PACK_MARGIN * 2;
        let fh = h + PACK_MARGIN * 2;
        let (px, py) = self.packer.allocate(fw, fh)?;
        let x = px + PACK_MARGIN;
        let y = py + PACK_MARGIN;

        let page_w = PAGE_SIZE as usize;

        match content {
            SwashContent::Mask => {
                // 1 byte per pixel (alpha only)
                for row in 0..h {
                    for col in 0..w {
                        let sx = (row * w + col) as usize;
                        let idx = ((y + row) as usize) * page_w + (x + col) as usize;
                        let a = image_data[sx];
                        self.pixels[idx] = [0xFF, 0xFF, 0xFF, a];
                    }
                }
            }
            SwashContent::Color | SwashContent::SubpixelMask => {
                // 4 bytes per pixel (RGBA)
                for row in 0..h {
                    for col in 0..w {
                        let sx = ((row * w + col) * 4) as usize;
                        let idx = ((y + row) as usize) * page_w + (x + col) as usize;
                        self.pixels[idx] = [
                            image_data[sx],
                            image_data[sx + 1],
                            image_data[sx + 2],
                            image_data[sx + 3],
                        ];
                    }
                }
            }
        }

        self.dirty = true;
        Some((x, y))
    }

    fn upload_to_gpu(&mut self, gfx: &D3D11) -> Result<()> {
        if !self.dirty {
            return Ok(());
        }
        let context = &gfx.imm_context;
        let tex = &self.texture.texture;
        let pw = PAGE_SIZE;
        let ph = PAGE_SIZE;
        let row_pitch = pw * 4;
        let pixel_bytes: &[u8] = unsafe {
            std::slice::from_raw_parts(
                self.pixels.as_ptr() as *const u8,
                (pw as usize) * (ph as usize) * 4,
            )
        };
        unsafe {
            let box_ = D3D11_BOX {
                left: 0,
                top: 0,
                front: 0,
                right: pw,
                bottom: ph,
                back: 1,
            };
            context.UpdateSubresource(
                &ID3D11Resource::from(tex.clone()),
                0,
                Some(&box_),
                pixel_bytes.as_ptr() as *const _,
                row_pitch,
                0,
            );
        }
        self.dirty = false;
        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────
// GlyphLocation
// ─────────────────────────────────────────────────────────────────────

#[derive(Clone)]
struct GlyphLocation {
    page_idx: usize,
    atlas_x: u32,
    atlas_y: u32,
    w: u32,
    h: u32,
    left: i32, // bearing-x for sprite positioning
    top: i32,  // bearing-y (positive = up from baseline)
    birth_tick: u64,
}

// ─────────────────────────────────────────────────────────────────────
// TextLayout — reusable layout result
// ─────────────────────────────────────────────────────────────────────

/// Stores the result of text layout: a list of (CacheKey, base_position) pairs.
/// Can be reused to render the same text multiple times with different
/// transforms, colours, or layers (e.g. for drop-shadow).
#[derive(Clone)]
pub struct TextLayout {
    /// List of glyph instances: (cache_key, base_position).
    /// base_position is (glyph.x, run.line_y) relative to the original origin.
    pub(crate) glyphs: Vec<(cosmic_text::CacheKey, Vec2)>,
}

// ─────────────────────────────────────────────────────────────────────
// AtlasText
// ─────────────────────────────────────────────────────────────────────

pub struct AtlasText {
    font_system: FontSystem,
    scale_context: ScaleContext,
    pages: Vec<TextPage>,
    glyph_cache: HashMap<cosmic_text::CacheKey, GlyphLocation>,
    pub lifetime_a: f32,
    pub lifetime_b: f32,
    tick: u64,
}

impl AtlasText {
    pub fn new(device: &ID3D11Device, lifetime_a: f32, lifetime_b: f32) -> Result<Self> {
        let font_system = FontSystem::new();
        let scale_context = ScaleContext::new();
        let first_page = TextPage::new(device)?;
        Ok(Self {
            font_system,
            scale_context,
            pages: vec![first_page],
            glyph_cache: HashMap::new(),
            lifetime_a,
            lifetime_b,
            tick: 0,
        })
    }

    pub fn page_count(&self) -> usize {
        self.pages.len()
    }
    pub fn texture_info(&self, pi: usize) -> &TextureInfo {
        &self.pages[pi].texture
    }
    pub fn texture_arced(&self, pi: usize) -> TextureInfoArced {
        TextureInfoArced(self.pages[pi].texture.clone())
    }
    pub fn set_lifetime_params(&mut self, a: f32, b: f32) {
        self.lifetime_a = a;
        self.lifetime_b = b;
    }

    pub fn evict_expired(&mut self) {
        if self.glyph_cache.is_empty() {
            return;
        }
        let tick = self.tick;
        let a = self.lifetime_a;
        let b = self.lifetime_b;
        let mut expired = Vec::new();
        for (key, loc) in &self.glyph_cache {
            let max_side = loc.w.max(loc.h) as f32;
            let lifetime = (max_side * a + b).max(1.0) as u64;
            if tick.saturating_sub(loc.birth_tick) > lifetime {
                expired.push(*key);
            }
        }
        for key in &expired {
            self.glyph_cache.remove(key);
        }
    }

    fn rasterize_glyph(&mut self, cache_key: cosmic_text::CacheKey) -> Option<SwashImage> {
        let Some(font) = self
            .font_system
            .get_font(cache_key.font_id, cache_key.font_weight)
        else {
            return None;
        };
        let mut scaler = self
            .scale_context
            .builder(font.as_swash())
            .size(f32::from_bits(cache_key.font_size_bits))
            .hint(
                !cache_key
                    .flags
                    .contains(cosmic_text::CacheKeyFlags::DISABLE_HINTING),
            )
            .build();
        let offset = Vector::new(cache_key.x_bin.as_float(), cache_key.y_bin.as_float());
        Render::new(&[
            Source::ColorOutline(0),
            Source::ColorBitmap(swash::scale::StrikeWith::BestFit),
            Source::Outline,
        ])
        .format(Format::Alpha)
        .offset(offset)
        .render(&mut scaler, cache_key.glyph_id)
    }

    /// Layout text and return a reusable `TextLayout`.
    /// Glyphs are rasterized and packed into the atlas (cache misses handled).
    /// The layout stores per-glyph base positions which can be offset later.
    pub fn layout_text(
        &mut self,
        text: &str,
        metrics: Metrics,
        attrs: Attrs,
        shaping: Shaping,
        device: &ID3D11Device,
    ) -> Result<TextLayout> {
        // Phase 1: layout
        let glyphs: Vec<(cosmic_text::CacheKey, Vec2)> = {
            let mut buf = Buffer::new(&mut self.font_system, metrics);
            let mut buf = buf.borrow_with(&mut self.font_system);
            buf.set_size(Some(f32::MAX), Some(f32::MAX));
            buf.set_text(text, &attrs, shaping, None);
            let mut info = Vec::new();
            for run in buf.layout_runs() {
                for glyph in run.glyphs.iter() {
                    let physical = glyph.physical((0.0, 0.0), 1.0);
                    let base_pos = Vec2::new(glyph.x, run.line_y);
                    info.push((physical.cache_key, base_pos));
                }
            }
            info
        };
        // buf dropped

        // Phase 2: rasterize any new glyphs and pack into atlas
        for (cache_key, _) in &glyphs {
            if self.glyph_cache.contains_key(cache_key) {
                continue;
            }
            let Some(image) = self.rasterize_glyph(*cache_key) else {
                continue;
            };
            let pw = image.placement.width as u32;
            let ph = image.placement.height as u32;
            if pw == 0 || ph == 0 {
                continue;
            }

            let loc = self
                .allocate_in_existing_pages(pw, ph, &image.data, image.content)
                .or_else(|| {
                    self.add_new_page(device).ok()?;
                    self.allocate_in_existing_pages(pw, ph, &image.data, image.content)
                });
            if let Some(mut loc) = loc {
                loc.left = image.placement.left;
                loc.top = image.placement.top;
                self.glyph_cache.insert(*cache_key, loc);
            }
        }

        Ok(TextLayout { glyphs })
    }

    /// Render a previously laid-out `TextLayout` into the sprite buffer.
    ///
    /// # Parameters
    /// - `layout` — layout result from `layout_text()`
    /// - `offset` — additional position offset (e.g. `(2,2)` for shadow)
    /// - `origin` — text origin point in layout space; `(0,0)` = top-left of first glyph,
    ///              `cx,cy` can centre the text, etc.
    /// - `transform` — per-glyph transform (scale, rotation is applied around each glyph's origin)
    /// - `color` — RGBA colour
    /// - `layer` — sort layer
    /// - `buffer` — target sprite buffer
    pub fn render_layout(
        &self,
        layout: &TextLayout,
        offset: Vec2,
        origin: Vec2,
        transform: Transform2D,
        color: [f32; 4],
        layer: f64,
        buffer: &mut Sprite2DBuffer<TextureInfoArced, Transform2D>,
    ) {
        for (cache_key, base_pos) in &layout.glyphs {
            if let Some(loc) = self.glyph_cache.get(cache_key) {
                let page = &self.pages[loc.page_idx];
                let pipeline = TextureInfoArced(page.texture.clone());

                let sprite_x = offset.x + base_pos.x + loc.left as f32 - origin.x;
                let sprite_y = offset.y + base_pos.y - loc.top as f32 - origin.y;

                let obj = Sprite2DObject {
                    spr: Sprite2D {
                        origin_px: Vec2::ZERO,
                        size_px: Vec2::new(loc.w as f32, loc.h as f32),
                        uv_tl_px: Vec2::new(loc.atlas_x as f32, loc.atlas_y as f32),
                        uv_size_px: Vec2::new(loc.w as f32, loc.h as f32),
                    },
                    color,
                    transform: Transform2D {
                        pos: Vec2::new(sprite_x, sprite_y),
                        scale: transform.scale,
                        rot: transform.rot,
                    },
                    pipeline,
                    layer,
                };
                buffer.push(&obj);
            }
        }
    }

    /// Convenience: render with identity transform and zero origin.
    pub fn render_layout_simple(
        &self,
        layout: &TextLayout,
        offset: Vec2,
        color: [f32; 4],
        layer: f64,
        buffer: &mut Sprite2DBuffer<TextureInfoArced, Transform2D>,
    ) {
        self.render_layout(
            layout,
            offset,
            Vec2::ZERO,
            Transform2D::IDENTITY,
            color,
            layer,
            buffer,
        );
    }

    /// One-shot convenience: layout + render in a single call.
    pub fn render_text(
        &mut self,
        text: &str,
        metrics: Metrics,
        attrs: Attrs,
        shaping: Shaping,
        offset: Vec2,
        color: [f32; 4],
        layer: f64,
        buffer: &mut Sprite2DBuffer<TextureInfoArced, Transform2D>,
        device: &ID3D11Device,
    ) -> Result<()> {
        let layout = self.layout_text(text, metrics, attrs, shaping, device)?;
        self.render_layout_simple(&layout, offset, color, layer, buffer);
        Ok(())
    }

    pub fn upload(&mut self, gfx: &D3D11) -> Result<()> {
        self.evict_expired();
        for page in &mut self.pages {
            if page.dirty {
                page.upload_to_gpu(gfx)?;
            }
        }
        self.tick = self.tick.wrapping_add(1);
        Ok(())
    }

    pub fn clear(&mut self, device: &ID3D11Device) -> Result<()> {
        self.glyph_cache.clear();
        self.pages.clear();
        self.pages.push(TextPage::new(device)?);
        Ok(())
    }

    fn allocate_in_existing_pages(
        &mut self,
        w: u32,
        h: u32,
        data: &[u8],
        content: SwashContent,
    ) -> Option<GlyphLocation> {
        for (page_idx, page) in self.pages.iter_mut().enumerate() {
            if let Some((x, y)) = page.allocate_and_write(w, h, data, content) {
                return Some(GlyphLocation {
                    page_idx,
                    atlas_x: x,
                    atlas_y: y,
                    w,
                    h,
                    left: 0,
                    top: 0,
                    birth_tick: self.tick,
                });
            }
        }
        None
    }

    fn add_new_page(&mut self, device: &ID3D11Device) -> Result<()> {
        self.pages.push(TextPage::new(device)?);
        Ok(())
    }
}
