use anyhow::{Context, Result};
use glam::Vec2;
use krjw_engine::{AtlasText, ShapeBatch2D, Sprite2D, Sprite2DBuffer, Sprite2DObject, SpriteBatch2D, TextureInfoArced, Transform2D, atlas_text::TextLayout, cosmic_text::{self, Attrs, Metrics}, graphic};

const HELP_TXT: &str = include_str!("help.txt");

pub struct HelperWindow {
    layout: TextLayout,
    time: f32,
}

impl HelperWindow {
    pub fn new(atlas_text: &mut AtlasText, gfx: &graphic::d3d11::D3D11) -> Result<Self> {
        let metrics = Metrics::new(16.0, 20.0);
        let attrs = Attrs::new().family(cosmic_text::Family::Name("SimHei"));
        let layout = atlas_text.layout_text(HELP_TXT, metrics, attrs, &gfx.device).context("layout_text failed")?;
        Ok(Self {
            layout,
            time: 0.0,
        })
    }

    pub fn render(&mut self, dt: f32, vp_size: Vec2, atlas_text: &mut AtlasText, sprite_buf: &mut Sprite2DBuffer<TextureInfoArced, Transform2D>, sprite_batch: &mut SpriteBatch2D, shape_batch: &mut ShapeBatch2D, gfx: &graphic::d3d11::D3D11) -> Result<()> {
        if self.time < 10.0 {
            let alpha = if self.time < 5.0 { 1.0 } else {
                (10.0 - self.time) / 5.0
            };
            let tr = vp_size * Vec2::new(0.5, -0.5);
            let offset = Vec2::new(-16.0, 16.0);
            let margin = 6.0_f32;
            let content_size = self.layout.content_size;
            let text_pos = tr + offset + Vec2::new(-margin, margin) - Vec2::new(content_size.x, -10.0);
            let frame_pos = tr + offset + Vec2::new(-margin, margin) * 2.0 - Vec2::new(content_size.x, 0.0);
            let frame_size = content_size + Vec2::splat(margin) * 2.0;
            shape_batch.add_rect_no_uv(frame_pos, frame_size, Vec2::ZERO, 0.0, [0.0, 0.0, 0.0, 0.75 * alpha]);
            shape_batch.submit_and_draw(gfx)?;
            shape_batch.clear_batch();
            atlas_text.render_layout(&self.layout, Vec2::ZERO, Vec2::ZERO, Transform2D::IDENTITY.move_by(text_pos), 
                [1.0, 1.0, 1.0, alpha], -1.0, sprite_buf);
            atlas_text.upload(&gfx)?;
            sprite_batch.draw_buffer_and_clear(gfx, sprite_buf, |x| (x.pos, x.scale, x.rot));
            self.time += dt;
        }
        else {}
        Ok(())
    }
}