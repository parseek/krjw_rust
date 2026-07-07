use anyhow::Result;
use glam::Vec2;
use windows::{
    Win32::Graphics::{Direct3D::*, Direct3D11::*, Dxgi::Common::*},
    core::PCSTR,
};

use super::d3d11_utils::{self, create_input_layout, write_buffer};
use super::D3D11;

#[allow(unused)]
#[derive(Copy, Clone)]
#[repr(C)]
struct ShapeVertex {
    pos: [f32; 2],
    color: [f32; 4],
}

#[allow(unused)]
#[repr(C)]
struct CbWorld {
    mvp: glam::Mat4,
}

#[allow(unused)]
const INPUT_LAYOUT_DESC: [D3D11_INPUT_ELEMENT_DESC; 2] = [
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
        SemanticName: PCSTR(b"COLOR\0".as_ptr()),
        SemanticIndex: 0,
        Format: DXGI_FORMAT_R32G32B32A32_FLOAT,
        InputSlot: 0,
        AlignedByteOffset: 8,
        InputSlotClass: D3D11_INPUT_PER_VERTEX_DATA,
        InstanceDataStepRate: 0,
    },
];

/// A batch renderer for solid-color 2D shapes.
///
/// Supports: `add_rect`, `add_circle`, `add_square_line`, `add_polygon`.
/// Each shape's transform (pos/scale/rot) is CPU-baked into the vertices.
///
/// `capacity` = maximum number of triangles per GPU batch.
/// If more triangles are added they are split into multiple draw calls
/// automatically.
#[allow(unused)]
pub struct ShapeBatch2D {
    vertex_shader: ID3D11VertexShader,
    pixel_shader: ID3D11PixelShader,
    input_layout: ID3D11InputLayout,

    vertex_buffer: ID3D11Buffer,
    index_buffer: ID3D11Buffer,
    cb_world: ID3D11Buffer,

    capacity: usize,

    vertices: Vec<ShapeVertex>,
    indices: Vec<[u16; 3]>,
}

#[allow(unused)]
impl ShapeBatch2D {
    /// Create a new shape batch.
    ///
    /// `capacity` — maximum triangles uploaded to the GPU per draw.
    /// GPU buffers are sized for `3 * capacity` vertices and `capacity` triangles.
    pub fn new(device: &ID3D11Device, capacity: usize) -> Result<Self> {
        let vs_blob = d3d11_utils::compile_shader(
            include_bytes!("shape_batch_2d_vs.hlsl"),
            PCSTR(b"main\0".as_ptr()),
            PCSTR(b"vs_5_0\0".as_ptr()),
        )?;

        let vertex_shader =
            d3d11_utils::create_vs(device, include_bytes!("shape_batch_2d_vs.hlsl"))?;
        let pixel_shader =
            d3d11_utils::create_ps(device, include_bytes!("shape_batch_2d_ps.hlsl"))?;
        let input_layout = create_input_layout(device, &INPUT_LAYOUT_DESC, &vs_blob)?;

        let vb_stride = std::mem::size_of::<ShapeVertex>() as u32;
        let vertex_buffer = d3d11_utils::create_dynamic_buffer(
            device,
            vb_stride * 3 * capacity as u32,
            D3D11_BIND_VERTEX_BUFFER.0 as u32,
        )?;

        let index_buffer = d3d11_utils::create_dynamic_buffer(
            device,
            (std::mem::size_of::<u16>() * 3 * capacity) as u32,
            D3D11_BIND_INDEX_BUFFER.0 as u32,
        )?;

        let cb_world = d3d11_utils::create_constant_buffer::<CbWorld>(device)?;

        Ok(Self {
            vertex_shader,
            pixel_shader,
            input_layout,
            vertex_buffer,
            index_buffer,
            cb_world,
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

    /// Add a rectangle.
    ///
    /// Transform: rotate → scale → translate.
    pub fn add_rect(&mut self, pos: Vec2, size: Vec2, rot: f32, color: [f32; 4]) {
        let (cos, sin) = rot.sin_cos();
        let hw = size.x * 0.5;
        let hh = size.y * 0.5;

        // Local corners (centered)
        let local: [[f32; 2]; 4] = [[-hw, -hh], [hw, -hh], [-hw, hh], [hw, hh]];
        let base = self.vertices.len() as u16;

        for &[lx, ly] in &local {
            let fx = (lx * cos - ly * sin) + pos.x;
            let fy = (lx * sin + ly * cos) + pos.y;
            self.vertices.push(ShapeVertex {
                pos: [fx, fy],
                color,
            });
        }

        self.indices.push([base, base + 1, base + 2]);
        self.indices.push([base + 2, base + 1, base + 3]);
    }

    /// Add a circle approximated as a triangle fan.
    ///
    /// `segments` — number of triangles (≥ 3).
    /// `rot` is ignored — circles are rotation-invariant.
    pub fn add_circle(
        &mut self,
        pos: Vec2,
        radius: f32,
        color: [f32; 4],
        segments: u32,
    ) {
        let segments = segments.max(3);
        let base = self.vertices.len() as u16;

        // Center vertex
        self.vertices.push(ShapeVertex {
            pos: [pos.x, pos.y],
            color,
        });

        // Edge vertices
        for i in 0..segments {
            let angle = (i as f32 / segments as f32) * std::f32::consts::TAU;
            let (s, c) = angle.sin_cos();
            self.vertices.push(ShapeVertex {
                pos: [pos.x + c * radius, pos.y + s * radius],
                color,
            });
        }

        // Fan triangles
        for i in 0..segments {
            let a = base;
            let b = base + 1 + i as u16;
            let c = base + 1 + ((i + 1) % segments) as u16;
            self.indices.push([a, b, c]);
        }
    }

    /// Add a thick line segment (rendered as a quad).
    pub fn add_square_line(
        &mut self,
        from: Vec2,
        to: Vec2,
        thickness: f32,
        color: [f32; 4],
    ) {
        let dir = to - from;
        let len = dir.length();
        if len < 1e-6 {
            return;
        }
        let ndir = dir / len;
        let perp = Vec2::new(-ndir.y, ndir.x) * (thickness * 0.5);

        let base = self.vertices.len() as u16;

        self.vertices.push(ShapeVertex {
            pos: [(from + perp).x, (from + perp).y],
            color,
        });
        self.vertices.push(ShapeVertex {
            pos: [(from - perp).x, (from - perp).y],
            color,
        });
        self.vertices.push(ShapeVertex {
            pos: [(to + perp).x, (to + perp).y],
            color,
        });
        self.vertices.push(ShapeVertex {
            pos: [(to - perp).x, (to - perp).y],
            color,
        });

        self.indices.push([base, base + 1, base + 2]);
        self.indices.push([base + 2, base + 1, base + 3]);
    }

    /// Add a convex polygon (triangle fan).
    ///
    /// `points` — vertices in CCW order, at least 3.
    /// The batch takes **ownership** of the vertex data during `submit_and_draw`.
    pub fn add_polygon(&mut self, points: &[Vec2], color: [f32; 4]) {
        let n = points.len();
        if n < 3 {
            return;
        }
        let base = self.vertices.len() as u16;

        for &p in points {
            self.vertices.push(ShapeVertex {
                pos: [p.x, p.y],
                color,
            });
        }

        // Triangle fan: [0, 1, 2], [0, 2, 3], [0, 3, 4], ...
        for i in 1..(n as u16 - 1) {
            self.indices.push([base, base + i, base + i + 1]);
        }
    }

    /// Upload the MVP matrix to the constant buffer.
    pub fn set_mvp(&self, gfx: &D3D11, mvp: &glam::Mat4) {
        write_buffer(&gfx.imm_context, &self.cb_world, &[CbWorld { mvp: *mvp }])
            .expect("shape_batch_2d::set_mvp failed");
    }

    /// Number of triangles currently in the batch.
    pub fn count(&self) -> usize {
        self.indices.len()
    }

    /// Draw all accumulated shapes, automatically splitting into
    /// multiple draw calls when the triangle count exceeds `capacity`.
    pub fn submit_and_draw(&mut self, gfx: &D3D11) -> Result<()> {
        let total_tris = self.indices.len();
        if total_tris == 0 {
            return Ok(());
        }

        let context = &gfx.imm_context;
        let rtv = gfx.rtv();
        let dsv = gfx.dsv();

        // Bind invariant state
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
            context.IASetIndexBuffer(&self.index_buffer, DXGI_FORMAT_R16_UINT, 0);

            context.VSSetConstantBuffers(0, Some(&[Some(self.cb_world.clone())]));
            context.VSSetShader(&self.vertex_shader, None);
            context.PSSetShader(&self.pixel_shader, None);

            context.OMSetRenderTargets(Some(&[Some(rtv.clone())]), Some(dsv));
        }

        // Process in batches of `capacity` triangles
        let mut tri_start = 0;
        while tri_start < total_tris {
            let tri_end = (tri_start + self.capacity).min(total_tris);
            let chunk_tris = &self.indices[tri_start..tri_end];

            // Collect unique vertices referenced by this chunk
            let mut local_verts: Vec<ShapeVertex> = Vec::with_capacity(3 * self.capacity);
            let mut remap: Vec<Option<u16>> = vec![None; self.vertices.len()];
            let mut local_indices: Vec<[u16; 3]> = Vec::with_capacity(chunk_tris.len());

            for tri in chunk_tris {
                let mut local_tri = [0u16; 3];
                for (j, &vi) in tri.iter().enumerate() {
                    if remap[vi as usize].is_none() {
                        remap[vi as usize] = Some(local_verts.len() as u16);
                        local_verts.push(self.vertices[vi as usize]);
                    }
                    local_tri[j] = remap[vi as usize].unwrap();
                }
                local_indices.push(local_tri);
            }

            // Upload vertices for this chunk
            write_buffer(&gfx.imm_context, &self.vertex_buffer, &local_verts)?;

            // Upload indices for this chunk
            write_buffer(&gfx.imm_context, &self.index_buffer, &local_indices)?;

            unsafe {
                context.DrawIndexed((3 * local_indices.len()) as u32, 0, 0);
            }

            tri_start = tri_end;
        }

        Ok(())
    }

    /// Convenience method: set_mvp → submit_and_draw.
    /// You still need to call `clear_batch` manually before reusing.
    pub fn draw(&mut self, gfx: &D3D11, mvp: &glam::Mat4) -> Result<()> {
        self.set_mvp(gfx, mvp);
        self.submit_and_draw(gfx)
    }
}