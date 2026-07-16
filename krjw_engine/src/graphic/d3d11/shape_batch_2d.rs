use anyhow::Result;
use glam::Vec2;
use windows::{
    Win32::Graphics::{Direct3D::*, Direct3D11::*, Dxgi::Common::*},
    core::PCSTR,
};

use super::D3D11;
use super::d3d11_utils::{self, write_buffer};
use super::state_objects::StateObjects;

#[allow(unused)]
#[derive(Copy, Clone)]
#[repr(C)]
pub struct ShapeVertex {
    pos: [f32; 2],
    uv: [f32; 2],
    color: [f32; 4],
}

#[allow(unused)]
#[repr(C)]
struct CbWorld {
    mvp: glam::Mat4,
}

/// A batch renderer for 2D shapes.
///
/// Methods without `_no_uv` suffix require a texture set via `set_texture`.
/// Methods with `_no_uv` suffix ignore UV and work with `ps_solid`.
///
/// `capacity` = maximum number of triangles per GPU batch.
/// If more triangles are added they are split into multiple draw calls.
#[allow(unused)]
pub struct ShapeBatch2D {
    vertex_shader: ID3D11VertexShader,
    pixel_shader: ID3D11PixelShader,
    input_layout: ID3D11InputLayout,

    vertex_buffer: ID3D11Buffer,
    index_buffer: ID3D11Buffer,
    cb_world: ID3D11Buffer,

    texture: Option<(ID3D11ShaderResourceView, u32, u32)>,

    capacity: usize,

    vertices: Vec<ShapeVertex>,
    indices: Vec<[u32; 3]>,
}

#[allow(unused)]
impl ShapeBatch2D {
    /// Create a new shape batch.
    ///
    /// `capacity` — maximum triangles per GPU draw (VB = 3*capacity, IB = capacity triangles).
    /// Default shaders: `vs_puc_m_2d`, `ps_solid_2d` (no texture required).
    pub fn new(
        device: &ID3D11Device,
        capacity: usize,
        vs: &ID3D11VertexShader,
        ps: &ID3D11PixelShader,
        input_layout: &ID3D11InputLayout,
    ) -> Result<Self> {
        assert!(capacity < (0xffffffff / 3 + 1));
        let vb_stride = std::mem::size_of::<ShapeVertex>() as u32;
        let vertex_buffer = d3d11_utils::create_dynamic_buffer(
            device,
            vb_stride * 3 * capacity as u32,
            D3D11_BIND_VERTEX_BUFFER.0 as u32,
        )?;

        let index_buffer = d3d11_utils::create_dynamic_buffer(
            device,
            (std::mem::size_of::<u32>() * 3 * capacity) as u32,
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
            vertices: Vec::new(),
            indices: Vec::new(),
        })
    }

    /// Clear accumulated shapes — call at the start of each frame.
    pub fn clear_batch(&mut self) {
        self.vertices.clear();
        self.indices.clear();
    }

    pub fn set_texture(&mut self, srv: ID3D11ShaderResourceView, width: u32, height: u32) {
        self.texture = Some((srv, width, height));
    }

    pub fn set_vertex_shader(&mut self, vs: ID3D11VertexShader) {
        self.vertex_shader = vs;
    }

    pub fn set_pixel_shader(&mut self, ps: ID3D11PixelShader) {
        self.pixel_shader = ps;
    }

    // ── No-UV methods (ignore texture, work with ps_solid) ──────

    /// Add a rectangle.
    ///
    /// - `pos` — position of the `origin_px` point (in world space)
    /// - `size` — rectangle size
    /// - `origin_px` — offset in pixels relative to the rectangle's top-left corner.
    ///   `(0,0)` = top-left, `(w/2, h/2)` = centre, `(w, h)` = bottom-right.
    /// - `rot` — rotation (radians) around `pos`
    /// - `color` — RGBA
    pub fn add_rect_no_uv(&mut self, pos: Vec2, size: Vec2, origin_px: Vec2, rot: f32, color: [f32; 4]) {
        let (sin, cos) = rot.sin_cos();
        // centre of the rect in local space
        let hw = size.x * 0.5;
        let hh = size.y * 0.5;
        let cx = -origin_px.x + hw;
        let cy = -origin_px.y + hh;
        // vertices relative to centre
        let local: [[f32; 2]; 4] = [[-hw, -hh], [hw, -hh], [-hw, hh], [hw, hh]];
        let base = self.vertices.len() as u32;

        for &[lx, ly] in &local {
            // translate to world: centre → world then rotate around pos
            let fx = (lx * cos - ly * sin) + pos.x + cx;
            let fy = (lx * sin + ly * cos) + pos.y + cy;
            self.vertices.push(ShapeVertex {
                pos: [fx, fy],
                uv: [0.0; 2],
                color,
            });
        }
        self.indices.push([base, base + 1, base + 2]);
        self.indices.push([base + 2, base + 1, base + 3]);
    }

    pub fn add_circle_no_uv(&mut self, pos: Vec2, radius: f32, color: [f32; 4], segments: u32) {
        let segments = segments.max(3);
        let base = self.vertices.len() as u32;

        self.vertices.push(ShapeVertex {
            pos: [pos.x, pos.y],
            uv: [0.0; 2],
            color,
        });
        for i in 0..segments {
            let angle = (i as f32 / segments as f32) * std::f32::consts::TAU;
            let (s, c) = angle.sin_cos();
            self.vertices.push(ShapeVertex {
                pos: [pos.x + c * radius, pos.y + s * radius],
                uv: [0.0; 2],
                color,
            });
        }
        for i in 0..segments {
            self.indices.push([
                base,
                base + 1 + i as u32,
                base + 1 + ((i + 1) % segments) as u32,
            ]);
        }
    }

    pub fn add_square_line_no_uv(&mut self, from: Vec2, to: Vec2, thickness: f32, color: [f32; 4]) {
        let dir = to - from;
        let len = dir.length();
        if len < 1e-6 {
            return;
        }
        let ndir = dir / len;
        let perp = Vec2::new(-ndir.y, ndir.x) * (thickness * 0.5);
        let base = self.vertices.len() as u32;

        for p in [from + perp, from - perp, to + perp, to - perp] {
            self.vertices.push(ShapeVertex {
                pos: [p.x, p.y],
                uv: [0.0; 2],
                color,
            });
        }
        self.indices.push([base, base + 1, base + 2]);
        self.indices.push([base + 2, base + 1, base + 3]);
    }

    pub fn add_polygon_no_uv(&mut self, points: &[Vec2], color: [f32; 4]) {
        let n = points.len();
        if n < 3 {
            return;
        }
        let base = self.vertices.len() as u32;
        for &p in points {
            self.vertices.push(ShapeVertex {
                pos: [p.x, p.y],
                uv: [0.0; 2],
                color,
            });
        }
        for i in 1..(n as u32 - 1) {
            self.indices.push([base, base + i, base + i + 1]);
        }
    }

    // ── UV methods (require texture via set_texture) ───────────

    /// Add a textured rectangle.
    ///
    /// - `pos` — position of the `origin_px` point (in world space)
    /// - `size` — rectangle size
    /// - `origin_px` — offset in pixels from top-left (0,0)=TL, (w/2,h/2)=centre, (w,h)=BR
    /// - `rot` — rotation (radians) around `pos`
    /// - `uv_tl_px`, `uv_size_px` — UV rectangle in pixels
    /// - `color` — RGBA tint
    pub fn add_rect(
        &mut self,
        pos: Vec2,
        size: Vec2,
        origin_px: Vec2,
        rot: f32,
        uv_tl_px: Vec2,
        uv_size_px: Vec2,
        color: [f32; 4],
    ) {
        let (tw, th) = match self.texture.as_ref() {
            Some((_, w, h)) => (*w as f32, *h as f32),
            None => {
                (uv_size_px.x, uv_size_px.y)
            }
        };
        let (sin, cos) = rot.sin_cos();
        let hw = size.x * 0.5;
        let hh = size.y * 0.5;
        let cx = -origin_px.x + hw;
        let cy = -origin_px.y + hh;
        let local: [[f32; 2]; 4] = [[-hw, -hh], [hw, -hh], [-hw, hh], [hw, hh]];
        let u0 = uv_tl_px.x / tw;
        let v0 = uv_tl_px.y / th;
        let u1 = (uv_tl_px.x + uv_size_px.x) / tw;
        let v1 = (uv_tl_px.y + uv_size_px.y) / th;
        let uvs = [[u0, v0], [u1, v0], [u0, v1], [u1, v1]];
        let base = self.vertices.len() as u32;

        for i in 0..4 {
            let (lx, ly) = (local[i][0], local[i][1]);
            let fx = (lx * cos - ly * sin) + pos.x + cx;
            let fy = (lx * sin + ly * cos) + pos.y + cy;
            self.vertices.push(ShapeVertex {
                pos: [fx, fy],
                uv: uvs[i],
                color,
            });
        }
        self.indices.push([base, base + 1, base + 2]);
        self.indices.push([base + 2, base + 1, base + 3]);
    }

    pub fn add_circle(
        &mut self,
        pos: Vec2,
        radius: f32,
        uv_tl_px: Vec2,
        uv_size_px: Vec2,
        color: [f32; 4],
        segments: u32,
    ) {
        let (tw, th) = match self.texture.as_ref() {
            Some((_, w, h)) => (*w as f32, *h as f32),
            None => (uv_size_px.x, uv_size_px.y),
        };
        let segments = segments.max(3);
        let base = self.vertices.len() as u32;
        let u0 = uv_tl_px.x / tw;
        let v0 = uv_tl_px.y / th;
        let u1 = (uv_tl_px.x + uv_size_px.x) / tw;
        let v1 = (uv_tl_px.y + uv_size_px.y) / th;

        // center vertex
        self.vertices.push(ShapeVertex {
            pos: [pos.x, pos.y],
            uv: [0.5 * (u0 + u1), 0.5 * (v0 + v1)],
            color,
        });
        for i in 0..segments {
            let angle = (i as f32 / segments as f32) * std::f32::consts::TAU;
            let (s, c) = angle.sin_cos();
            self.vertices.push(ShapeVertex {
                pos: [pos.x + c * radius, pos.y + s * radius],
                uv: [
                    0.5 * (u0 + u1) + 0.5 * (u1 - u0) * c,
                    0.5 * (v0 + v1) + 0.5 * (v1 - v0) * s,
                ],
                color,
            });
        }
        for i in 0..segments {
            self.indices.push([
                base,
                base + 1 + i as u32,
                base + 1 + ((i + 1) % segments) as u32,
            ]);
        }
    }

    // ── Common methods ─────────────────────────────────────────

    pub fn push(&mut self, vertices: &[ShapeVertex], tri_indicies: &[[u32; 3]]) {
        let len = vertices.len();
        let s_i = self.vertices.len();
        assert!(s_i < 0x10000);
        let s_i = s_i as u32;
        self.vertices.extend_from_slice(vertices);
        for i in tri_indicies {
            self.indices.push(i.map(|x| x + s_i));
        }
    }

    pub fn push_with_transform_2d(&mut self, vertices: &[ShapeVertex], tri_indicies: &[[u32; 3]], pos: Vec2, scale: Vec2, rot: f32) {
        let len = vertices.len();
        let s_i = self.vertices.len();
        assert!(s_i < 0x10000);
        let s_i = s_i as u32;
        let mut tr_vert = Vec::with_capacity(vertices.len());
        let (sin, cos) = rot.sin_cos();
        for i in vertices {
            let p = Vec2::from_slice(&i.pos) * scale;
            let p = Vec2::new(sin, cos).rotate(p);
            let p = p * scale;
            tr_vert.push(ShapeVertex {
                pos: [p.x, p.y],
                ..*i
            });
        }
        self.vertices.extend_from_slice(vertices);
        for i in tri_indicies {
            self.indices.push(i.map(|x| x + s_i));
        }
    }

    pub fn set_mvp(&self, gfx: &D3D11, mvp: &glam::Mat4) {
        write_buffer(&gfx.imm_context, &self.cb_world, &[CbWorld { mvp: *mvp }])
            .expect("shape_batch_2d::set_mvp failed");
    }

    pub fn count(&self) -> usize {
        self.indices.len()
    }

    pub fn submit_and_draw(&mut self, gfx: &D3D11) -> Result<()> {
        let total_tris = self.indices.len();
        if total_tris == 0 {
            return Ok(());
        }

        let context = &gfx.imm_context;
        let rtv = gfx.rtv();
        let dsv = gfx.dsv();

        unsafe {
            context.IASetInputLayout(&self.input_layout);
            context.IASetPrimitiveTopology(D3D11_PRIMITIVE_TOPOLOGY_TRIANGLELIST);

            let stride = std::mem::size_of::<ShapeVertex>() as u32;
            let offset = 0u32;
            context.IASetVertexBuffers(
                0,
                1,
                Some([Some(self.vertex_buffer.clone())].as_ptr()),
                Some([stride].as_ptr()),
                Some([offset].as_ptr()),
            );
            context.IASetIndexBuffer(&self.index_buffer, DXGI_FORMAT_R32_UINT, 0);

            context.VSSetConstantBuffers(0, Some(&[Some(self.cb_world.clone())]));
            context.VSSetShader(&self.vertex_shader, None);
            context.PSSetShader(&self.pixel_shader, None);

            // Bind texture if set
            if let Some((ref srv, _, _)) = self.texture {
                context.PSSetShaderResources(0, Some(&[Some(srv.clone())]));
            }

            context.OMSetRenderTargets(Some(&[Some(rtv.clone())]), Some(dsv));
        }

        // Process in batches of `capacity` triangles
        let mut tri_start = 0;
        while tri_start < total_tris {
            let tri_end = (tri_start + self.capacity).min(total_tris);
            let chunk_tris = &self.indices[tri_start..tri_end];

            let mut local_verts: Vec<ShapeVertex> = Vec::with_capacity(3 * self.capacity);
            let mut remap: Vec<Option<u32>> = vec![None; self.vertices.len()];
            let mut local_indices: Vec<[u32; 3]> = Vec::with_capacity(chunk_tris.len());

            for tri in chunk_tris {
                let mut local_tri = [0u32; 3];
                for (j, &vi) in tri.iter().enumerate() {
                    if remap[vi as usize].is_none() {
                        remap[vi as usize] = Some(local_verts.len() as u32);
                        local_verts.push(self.vertices[vi as usize]);
                    }
                    local_tri[j] = remap[vi as usize].unwrap();
                }
                local_indices.push(local_tri);
            }

            write_buffer(&gfx.imm_context, &self.vertex_buffer, &local_verts)?;
            write_buffer(&gfx.imm_context, &self.index_buffer, &local_indices)?;

            unsafe {
                context.DrawIndexed((3 * local_indices.len()) as u32, 0, 0);
            }

            tri_start = tri_end;
        }

        Ok(())
    }

    pub fn draw(&mut self, gfx: &D3D11, mvp: &glam::Mat4) -> Result<()> {
        self.set_mvp(gfx, mvp);
        self.submit_and_draw(gfx)
    }
}
