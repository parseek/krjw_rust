use anyhow::{Context, Error, Result};
use windows::{
    Win32::Graphics::{Direct3D::*, Direct3D11::*, Dxgi::Common::*},
    core::PCSTR,
};

use super::d3d11_utils::{self, create_input_layout, write_buffer};
use super::D3D11;

/// Describes the source rectangle of a sprite (in pixels).
#[allow(unused)]
#[derive(Copy, Clone)]
pub struct Sprite {
    pub origin_px: [f32; 2],
    pub size_px: [f32; 2],
    pub uv_tl_px: [f32; 2],
    pub uv_size_px: [f32; 2],
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
const INPUT_LAYOUT_DESC: [D3D11_INPUT_ELEMENT_DESC; 3] = [
    D3D11_INPUT_ELEMENT_DESC {
        SemanticName: PCSTR(b"POSITION\0".as_ptr()),
        SemanticIndex: 0,
        Format: DXGI_FORMAT_R32G32_FLOAT,
        InputSlot: 0,
        AlignedByteOffset: 0,
        InputSlotClass: D3D11_INPUT_PER_VERTEX_DATA,
        InstanceDataStepRate: 0,
    },
    D3D11_INPUT_ELEMENT_DESC {
        SemanticName: PCSTR(b"TEXCOORD\0".as_ptr()),
        SemanticIndex: 0,
        Format: DXGI_FORMAT_R32G32_FLOAT,
        InputSlot: 0,
        AlignedByteOffset: 8,
        InputSlotClass: D3D11_INPUT_PER_VERTEX_DATA,
        InstanceDataStepRate: 0,
    },
    D3D11_INPUT_ELEMENT_DESC {
        SemanticName: PCSTR(b"COLOR\0".as_ptr()),
        SemanticIndex: 0,
        Format: DXGI_FORMAT_R32G32B32A32_FLOAT,
        InputSlot: 0,
        AlignedByteOffset: 16,
        InputSlotClass: D3D11_INPUT_PER_VERTEX_DATA,
        InstanceDataStepRate: 0,
    },
];

#[derive(Debug)]
struct Texture {
    texture_srv: ID3D11ShaderResourceView,
    tex_width: u32,
    tex_height: u32,
}

#[allow(unused)]
pub struct SpriteBatch2D {
    vertex_shader: ID3D11VertexShader,
    pixel_shader: ID3D11PixelShader,
    input_layout: ID3D11InputLayout,

    vertex_buffer: ID3D11Buffer,
    index_buffer: ID3D11Buffer,
    cb_world: ID3D11Buffer,

    texture: Option<Texture>,

    capacity: usize,
    vertices: Vec<[SpriteVertex2D; 4]>,

    // Incremented on every add/clear_batch.
    // submit_and_draw compares this against drawn_version to decide
    // whether the GPU buffer needs to be re-uploaded.
    batch_version: u64,
    drawn_version: u64,
}

#[allow(unused)]
impl SpriteBatch2D {
    pub fn new(
        device: &ID3D11Device,
        capacity: usize,
    ) -> Result<Self> {
        if capacity > (0xffff/4) {
            return Err(Error::msg("capacity out of range"));
        }

        // ── Shaders ──────────────────────────────────────────────
        let vs_blob = d3d11_utils::compile_shader(
            include_bytes!("sprite_batch_2d_vs.hlsl"),
            PCSTR(b"main\0".as_ptr()),
            PCSTR(b"vs_5_0\0".as_ptr()),
        )?;

        let vertex_shader =
            d3d11_utils::create_vs(device, include_bytes!("sprite_batch_2d_vs.hlsl"))?;
        let pixel_shader =
            d3d11_utils::create_ps(device, include_bytes!("sprite_batch_2d_ps.hlsl"))?;
        let input_layout = create_input_layout(device, &INPUT_LAYOUT_DESC, &vs_blob)?;

        // ── Vertex buffer (dynamic) ──────────────────────────────
        let vb_stride = std::mem::size_of::<SpriteVertex2D>() as u32;
        let vertex_buffer = d3d11_utils::create_dynamic_buffer(
            device,
            vb_stride * 4 * capacity as u32,
            D3D11_BIND_VERTEX_BUFFER.0 as u32,
        )?;

        // ── Index buffer (immutable, pre-filled) ─────────────────
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

        // ── Constant buffer (mvp) ────────────────────────────────
        let cb_world = d3d11_utils::create_constant_buffer::<CbWorld>(device)?;

        Ok(Self {
            vertex_shader,
            pixel_shader,
            input_layout,
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

    /// Clear accumulated vertices — call at the start of each frame.
    pub fn clear_batch(&mut self) {
        self.vertices.clear();
        self.batch_version = self.batch_version.wrapping_add(1);
    }

    /// Set the texture
    pub fn set_texture(
        &mut self,
        texture_srv: ID3D11ShaderResourceView,
        tex_width: u32,
        tex_height: u32,
    ) {
        self.texture = Some(Texture { texture_srv, tex_width, tex_height });
    }

    /// Add one sprite. Transform = rotate → scale → translate.
    pub fn add(
        &mut self,
        pos: [f32; 2],
        scale: [f32; 2],
        rot: f32,
        sprite: &Sprite,
        color: [f32; 4],
    ) -> Result<()> {
        let tex = self.texture.as_ref().context("No texture")?;

        let Sprite {
            origin_px,
            size_px,
            uv_tl_px,
            uv_size_px,
        } = *sprite;

        let (cos, sin) = rot.sin_cos();
        let tw = tex.tex_width as f32;
        let th = tex.tex_height as f32;
        let u0 = uv_tl_px[0] / tw;
        let v0 = uv_tl_px[1] / th;
        let u1 = (uv_tl_px[0] + uv_size_px[0]) / tw;
        let v1 = (uv_tl_px[1] + uv_size_px[1]) / th;

        // 4 corner local positions (relative to origin)
        let ox = -origin_px[0];
        let oy = -origin_px[1];
        let corners: [[f32; 2]; 4] = [
            [ox, oy],
            [ox + size_px[0], oy],
            [ox, oy + size_px[1]],
            [ox + size_px[0], oy + size_px[1]],
        ];
        let uvs: [[f32; 2]; 4] = [[u0, v0], [u1, v0], [u0, v1], [u1, v1]];

        let mut quad = [SpriteVertex2D {
            pos: [0.0; 2],
            uv: [0.0; 2],
            color,
        }; 4];

        for i in 0..4 {
            let lx = corners[i][0];
            let ly = corners[i][1];
            // rotate → scale → translate
            let fx = (lx * cos - ly * sin) * scale[0] + pos[0];
            let fy = (lx * sin + ly * cos) * scale[1] + pos[1];

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

    /// Upload the MVP matrix to the constant buffer.
    pub fn set_mvp(&self, gfx: &D3D11, mvp: &glam::Mat4) {
        write_buffer(
            &gfx.imm_context,
            &self.cb_world,
            &[CbWorld { mvp: *mvp }],
        )
        .expect("sprite_batch_2d::set_mvp failed");
    }

    /// Draw all accumulated sprites, auto-submitting if the batch has
    /// changed since the last call.  Automatically splits into multiple
    /// draw calls when the sprite count exceeds `capacity`.
    ///
    /// When the batch has *not* changed (i.e. no `add` or `clear_batch`
    /// between two calls) the GPU data is simply re-used — useful for
    /// multi-pass / post-processing effects.
    pub fn submit_and_draw(&mut self, gfx: &D3D11) -> Result<()> {
        let tex = self.texture.as_ref().context("No texture")?;

        let total = self.vertices.len();
        if total == 0 {
            return Ok(());
        }

        let needs_submit = (self.batch_version != self.drawn_version) || (total > self.capacity);

        let context = &gfx.imm_context;
        let rtv = gfx.rtv();

        // Bind invariant state once (shaders, layout, textures, etc.)
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
            context.PSSetShaderResources(0, Some(&[Some(tex.texture_srv.clone())]));

            context.OMSetRenderTargets(Some(&[Some(rtv.clone())]), None);
        }

        if needs_submit {
            // Data changed — upload per chunk and draw.
            for chunk_start in (0..total).step_by(self.capacity) {
                let chunk_end = (chunk_start + self.capacity).min(total);
                let chunk = &self.vertices[chunk_start..chunk_end];
                let quad_count = chunk.len();
                debug_assert!(quad_count <= self.capacity);

                write_buffer(&gfx.imm_context, &self.vertex_buffer, chunk)
                    .unwrap_or_else(|e| panic!("sprite_batch_2d::submit failed: {:#}", e));

                unsafe {
                    context.DrawIndexed(6 * quad_count as u32, 0, 0);
                }
            }
        } else {
            // Reuse already-uploaded data — single draw for the full count.
            // (Only reaches 2nd+ pass in multi-pass effects; total ≤ capacity.)
            let quad_count = total as u32;
            unsafe {
                context.DrawIndexed(6 * quad_count, 0, 0);
            }
        }

        self.drawn_version = self.batch_version;
        Ok(())
    }

    /// Convenience method: clear_batch → (add …) → set_mvp → submit_and_draw.
    ///
    /// Like `submit_and_draw`, you have to *manually* clears the batch after drawing
    /// and then you can start collecting sprites for the next frame.
    pub fn draw(&mut self, gfx: &D3D11, mvp: &glam::Mat4) -> Result<()> {
        self.set_mvp(gfx, mvp);
        self.submit_and_draw(gfx)
    }

    /// Number of sprites currently in the batch.
    pub fn count(&self) -> usize {
        self.vertices.len()
    }
}