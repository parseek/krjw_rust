use anyhow::{Context, Error, Result};
use glam::Vec2;
use windows::{
    Win32::Graphics::{Direct3D::*, Direct3D11::*, Dxgi::Common::*},
    core::PCSTR,
};

use super::D3D11;
use super::d3d11_utils::{self, write_buffer};

/// Describes the source rectangle of a sprite (in pixels).
#[allow(unused)]
#[derive(Copy, Clone)]
pub struct Sprite {
    pub origin_px: Vec2,
    pub size_px: Vec2,
    pub uv_tl_px: Vec2,
    pub uv_size_px: Vec2,
}

#[allow(unused)]
#[derive(Copy, Clone)]
#[repr(C)]
struct SpriteVertex2D {
    pos: [f32; 2],
    uv: [f32; 2],
    color: [f32; 4],
}

#[allow(unused)]
#[repr(C)]
struct CbWorld {
    mvp: glam::Mat4,
}

#[allow(unused)]
pub struct SpriteBatch2D {
    vertex_shader: ID3D11VertexShader,
    pixel_shader: ID3D11PixelShader,
    input_layout: ID3D11InputLayout,

    vertex_buffer: ID3D11Buffer,
    index_buffer: ID3D11Buffer,
    cb_world: ID3D11Buffer,

    texture: Option<(ID3D11ShaderResourceView, u32, u32)>,

    capacity: usize,
    vertices: Vec<[SpriteVertex2D; 4]>,

    batch_version: u64,
    drawn_version: u64,
}

#[allow(unused)]
impl SpriteBatch2D {
    pub fn new(
        device: &ID3D11Device,
        capacity: usize,
        vs: &ID3D11VertexShader,
        ps: &ID3D11PixelShader,
        input_layout: &ID3D11InputLayout,
    ) -> Result<Self> {
        if capacity > (0xffff / 4) {
            return Err(Error::msg("capacity out of range"));
        }

        let vb_stride = std::mem::size_of::<SpriteVertex2D>() as u32;
        let vertex_buffer = d3d11_utils::create_dynamic_buffer(
            device,
            vb_stride * 4 * capacity as u32,
            D3D11_BIND_VERTEX_BUFFER.0 as u32,
        )?;

        let total_indices = 6 * capacity;
        let mut indices = Vec::<u16>::with_capacity(total_indices);
        for i in 0..capacity as u16 {
            let base = i * 4;
            indices.extend_from_slice(&[base, base + 1, base + 2, base + 2, base + 1, base + 3]);
        }
        let index_buffer = d3d11_utils::create_immutable_buffer(
            device,
            d3d11_utils::as_u8_slice(&indices),
            D3D11_BIND_INDEX_BUFFER.0 as u32,
        )?;

        let cb_world = d3d11_utils::create_constant_buffer::<CbWorld>(device)?;

        Ok(Self {
            vertex_shader: vs.clone(),
            pixel_shader: ps.clone(),
            input_layout: input_layout.clone(),
            vertex_buffer,
            index_buffer,
            cb_world,
            texture: None,
            capacity,
            vertices: Vec::with_capacity(capacity),
            batch_version: 0,
            drawn_version: 0,
        })
    }

    pub fn clear_batch(&mut self) {
        self.vertices.clear();
        self.batch_version = self.batch_version.wrapping_add(1);
    }

    pub fn set_texture(
        &mut self,
        texture_srv: ID3D11ShaderResourceView,
        tex_width: u32,
        tex_height: u32,
    ) {
        self.texture = Some((texture_srv, tex_width, tex_height));
    }

    pub fn set_vertex_shader(&mut self, vs: ID3D11VertexShader) {
        self.vertex_shader = vs;
    }

    pub fn set_pixel_shader(&mut self, ps: ID3D11PixelShader) {
        self.pixel_shader = ps;
    }

    /// Add one sprite. Transform = rotate → scale → translate.
    pub fn add(
        &mut self,
        pos: Vec2,
        scale: Vec2,
        rot: f32,
        sprite: &Sprite,
        color: [f32; 4],
    ) -> Result<()> {
        let (tex_srv, tex_width, tex_height) =
            self.texture.as_ref().context("No texture set")?;

        let Sprite {
            origin_px,
            size_px,
            uv_tl_px,
            uv_size_px,
        } = *sprite;

        let (sin, cos) = rot.sin_cos();
        let tw = *tex_width as f32;
        let th = *tex_height as f32;
        let u0 = uv_tl_px.x / tw;
        let v0 = uv_tl_px.y / th;
        let u1 = (uv_tl_px.x + uv_size_px.x) / tw;
        let v1 = (uv_tl_px.y + uv_size_px.y) / th;

        let ox = -origin_px.x;
        let oy = -origin_px.y;
        let corners: [[f32; 2]; 4] = [
            [ox, oy],
            [ox + size_px.x, oy],
            [ox, oy + size_px.y],
            [ox + size_px.x, oy + size_px.y],
        ];
        let uvs: [[f32; 2]; 4] = [[u0, v0], [u1, v0], [u0, v1], [u1, v1]];

        let mut quad = [SpriteVertex2D {
            pos: [0.0; 2],
            uv: [0.0; 2],
            color,
        }; 4];

        for i in 0..4 {
            let lx = corners[i][0] * scale.x;
            let ly = corners[i][1] * scale.y;
            let fx = (lx * cos - ly * sin) + pos.x;
            let fy = (lx * sin + ly * cos) + pos.y;

            quad[i] = SpriteVertex2D {
                pos: [fx, fy],
                uv: uvs[i],
                color,
            };
        }

        self.vertices.push(quad);
        self.batch_version = self.batch_version.wrapping_add(1);
        Ok(())
    }

    pub fn set_mvp(&self, gfx: &D3D11, mvp: &glam::Mat4) {
        write_buffer(&gfx.imm_context, &self.cb_world, &[CbWorld { mvp: *mvp }])
            .expect("sprite_batch_2d::set_mvp failed");
    }

    pub fn submit_and_draw(&mut self, gfx: &D3D11) -> Result<()> {
        let (tex_srv, ..) = self.texture.as_ref().context("No texture set")?;

        let total = self.vertices.len();
        if total == 0 {
            return Ok(());
        }

        let needs_submit = (self.batch_version != self.drawn_version) || (total > self.capacity);

        let context = &gfx.imm_context;
        let rtv = gfx.rtv();
        let dsv = gfx.dsv();

        unsafe {
            context.IASetInputLayout(&self.input_layout);
            context.IASetPrimitiveTopology(D3D11_PRIMITIVE_TOPOLOGY_TRIANGLELIST);

            let stride = std::mem::size_of::<SpriteVertex2D>() as u32;
            let offset = 0u32;
            context.IASetVertexBuffers(
                0,
                1,
                Some([Some(self.vertex_buffer.clone())].as_ptr()),
                Some([stride].as_ptr()),
                Some([offset].as_ptr()),
            );
            context.IASetIndexBuffer(&self.index_buffer, DXGI_FORMAT_R16_UINT, 0);

            context.VSSetConstantBuffers(0, Some(&[Some(self.cb_world.clone())]));
            context.VSSetShader(&self.vertex_shader, None);
            context.PSSetShader(&self.pixel_shader, None);
            context.PSSetShaderResources(0, Some(&[Some(tex_srv.clone())]));

            context.OMSetRenderTargets(Some(&[Some(rtv.clone())]), Some(dsv));
        }

        if needs_submit {
            for chunk_start in (0..total).step_by(self.capacity) {
                let chunk_end = (chunk_start + self.capacity).min(total);
                let chunk = &self.vertices[chunk_start..chunk_end];

                write_buffer(&gfx.imm_context, &self.vertex_buffer, chunk)
                    .unwrap_or_else(|e| panic!("sprite_batch_2d::submit failed: {:#}", e));

                unsafe {
                    context.DrawIndexed(6 * chunk.len() as u32, 0, 0);
                }
            }
        } else {
            unsafe {
                context.DrawIndexed(6 * total as u32, 0, 0);
            }
        }

        self.drawn_version = self.batch_version;
        Ok(())
    }

    pub fn draw(&mut self, gfx: &D3D11, mvp: &glam::Mat4) -> Result<()> {
        self.set_mvp(gfx, mvp);
        self.submit_and_draw(gfx)
    }

    pub fn count(&self) -> usize {
        self.vertices.len()
    }
}