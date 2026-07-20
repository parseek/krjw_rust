use std::sync::Arc;

use anyhow::Result;
use crate::platform::direct3d11::*;

use crate::graphic::d3d11::d3d11_utils;

#[derive(Clone, Copy, Debug, Default)]
#[repr(C)]
pub struct VertexP3U2C4 {
    pos: [f32; 3],
    uv: [f32; 2],
    color: [f32; 4],
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Debug, Clone, Copy)]
enum Indicies {
    QuadOnly,
    Polygon,
}

#[derive(PartialEq, Eq, Debug, Clone, Copy)]
enum PushResult {
    /// Don't have to reset
    Ok,

    /// Have to do DrawCall and reset
    Full,
}

struct BeginToDraw {
    vertices: Box<[VertexP3U2C4]>,
    pindicies: Box<[u16]>,
    indicies: Indicies,
    verts_count: u32,
    triangles: u32,
    polygoned_quads: u32,
}

impl BeginToDraw {
    /// 一次 `DrawCall` 最大的三角形数
    pub const DRAWPAGE_CAPACITY_TRIANGLES: usize = 2048;
    pub const QUAD_THRESHOLD: u32 = 16;
    
    /// 顶点缓存的容量
    pub const fn get_idx_capacity() -> usize { Self::DRAWPAGE_CAPACITY_TRIANGLES * 3 }
    pub const fn get_vert_capacity() -> usize { Self::DRAWPAGE_CAPACITY_TRIANGLES * 3 }

}

impl BeginToDraw {
    pub fn new() -> Self {
        Self {
            vertices: Box::new([VertexP3U2C4::default(); Self::get_vert_capacity()]),
            pindicies: Box::new([0; Self::get_vert_capacity()]),
            indicies: Indicies::QuadOnly,
            verts_count: 0,
            triangles: 0,
            polygoned_quads: 0,
        }
    }
    const QUAD_INDICIES: [u16; 6] = [0, 1, 3, 3, 2, 0];

    pub fn push_polygon(&mut self, vertices: &[VertexP3U2C4], indicies: &[u16]) -> PushResult {
        debug_assert!(vertices.len() <= indicies.len());
        debug_assert_eq!(indicies.len() % 3, 0);
        let input_tris = (indicies.len() / 3) as u32;
        if self.triangles + input_tris <= Self::DRAWPAGE_CAPACITY_TRIANGLES as u32 {
            if self.indicies == Indicies::QuadOnly {
                debug_assert_eq!(self.triangles % 2, 0);
                debug_assert_eq!(self.verts_count % 4, 0);
                if self.triangles > Self::QUAD_THRESHOLD*2 {
                    return PushResult::Full;
                }
                let quads = self.triangles / 2;
                for i in 0..quads {
                    let si = (i * 4) as u16;
                    let sv = (i * 6) as usize;
                    self.pindicies[sv + 0] = si + Self::QUAD_INDICIES[0];
                    self.pindicies[sv + 1] = si + Self::QUAD_INDICIES[1];
                    self.pindicies[sv + 2] = si + Self::QUAD_INDICIES[2];
                    self.pindicies[sv + 3] = si + Self::QUAD_INDICIES[3];
                    self.pindicies[sv + 4] = si + Self::QUAD_INDICIES[4];
                    self.pindicies[sv + 5] = si + Self::QUAD_INDICIES[5];
                }
            }
            let start_vert = &mut self.vertices[self.verts_count as usize];
            let start_indicies = self.triangles as usize * 3;
            let start_indicies = &mut self.pindicies[start_indicies..start_indicies+indicies.len()];
            let vert_slice = &mut self.vertices[self.verts_count as usize..self.verts_count as usize + vertices.len()];
            vert_slice.copy_from_slice(vertices);
            let base = self.verts_count as u16;
            start_indicies.iter_mut().zip(indicies.iter()).for_each(|(dst, &src)| *dst = src + base);
            self.verts_count += vertices.len() as u32;
            self.triangles += input_tris;
            self.indicies = Indicies::Polygon;
            PushResult::Ok
        } else {
            PushResult::Full
        }
    }
    pub fn push_quad(&mut self, vertices: &[VertexP3U2C4]) -> PushResult {
        debug_assert_eq!(vertices.len(), 4);
        if self.indicies == Indicies::Polygon {
            if self.polygoned_quads > Self::QUAD_THRESHOLD {
                return PushResult::Full;
            }
            self.polygoned_quads+=1;
            self.push_polygon(vertices, &Self::QUAD_INDICIES)
        } else {
            debug_assert!(vertices.len() <= Self::QUAD_INDICIES.len());
            debug_assert_eq!(Self::QUAD_INDICIES.len() % 3, 0);
            debug_assert_eq!(self.vertices.len() % 4, 0);
            let input_tris = (Self::QUAD_INDICIES.len() / 3) as u32;
            if self.triangles + input_tris <= Self::DRAWPAGE_CAPACITY_TRIANGLES as u32 {
                let start_vert = &mut self.vertices[self.verts_count as usize];
                // let start_indicies = &mut self.pindicies[self.triangles as usize * 3];
                let vert_slice = &mut self.vertices[self.verts_count as usize..self.verts_count as usize + vertices.len()];
                vert_slice.copy_from_slice(vertices);
                // unsafe { std::ptr::copy_nonoverlapping(Self::QUAD_INDICIES.as_ptr(), start_indicies, Self::QUAD_INDICIES.len());}
                self.verts_count += vertices.len() as u32;
                self.triangles += input_tris;
                PushResult::Ok
            } else {
                PushResult::Full
            }
        }
    }
    pub fn clear(&mut self) {
        self.indicies = Indicies::QuadOnly;
        self.triangles = 0;
        self.verts_count = 0;
        self.polygoned_quads = 0;
    }

    pub fn get_buf_refs(&self) -> (&[VertexP3U2C4], Option<&[u16]>) {
        (&self.vertices[0..self.verts_count as usize], match self.indicies { Indicies::Polygon => Some(&self.pindicies[0..self.triangles as usize * 3]), Indicies::QuadOnly => None })
    }
}

/// 确保顶点索引 <= u16::MAX
const _: () = assert!(BeginToDraw::get_vert_capacity() <= u16::MAX as usize);

#[derive(PartialEq, Eq, PartialOrd, Ord, Debug, Clone, Copy)]
pub struct SortKey(pub u128);

#[derive(PartialEq, Eq, PartialOrd, Ord, Debug, Clone, Copy)]
pub struct StateKey(pub u64);

impl StateKey {
    pub fn new(texture_id: u32, rstates_id: u32) -> Self {
        let texture_id = texture_id as u64;
        let rstates_id = rstates_id as u64;
        Self(texture_id << 32 | rstates_id)
    }
    pub fn texture_id(&self) -> u32 {
        (self.0 >> 32) as u32
    }
    pub fn rstates_id(&self) -> u32 {
        (self.0) as u32
    }
}

impl SortKey {
    pub fn new(z_index: u64, texture_id: u32, rstates_id: u32) -> Self {
        let z_index = z_index as u128;
        let texture_id = texture_id as u128;
        let rstates_id = rstates_id as u128;
        Self(z_index << 64 | texture_id << 32 | rstates_id)
    }
    pub fn z_index(&self) -> u64 {
        (self.0 >> 64) as u64
    }
    pub fn texture_id(&self) -> u32 {
        (self.0 >> 32) as u32
    }
    pub fn rstates_id(&self) -> u32 {
        (self.0) as u32
    }
    pub fn state_key(&self) -> StateKey {
        StateKey((self.0) as u64)
    }
}

#[repr(align(16))]
pub enum DrawContent {
    Quad {
        start_vert: u32,
    },
    Polygon {
        start_vert: u32,
        vert_count: u16,
        start_index: u32,
        index_count: u16,
    },
    Foregin(Arc<dyn Fn(&super::D3D11, SortKey) -> ()>),
}

// 可以利用 SIMD 复制？

#[repr(align(16))]
pub struct DrawCmd {
    pub key: SortKey,
    pub content: DrawContent,
}

/// 生成 DrawCmd、内部排序、（一次或多次）提交给 BeginToDraw 存储再将其缓冲区写入 GPU 缓冲、设置渲染状态、绘制
pub struct Batch2D {
    /// D3D11 GFX
    gfx: Arc<super::D3D11>,

    // TODO: 让 D3D11 允许可变
    // TODO: 加入 Texture、RStates（包括渲染状态、Shader等） 管理器

    /// `Quad`、`Polygon` 通用\
    /// 待排序、提交的顶点
    prepared_vertices: Vec<VertexP3U2C4>,

    /// `Polygon` 用的 **相对** 索引\
    /// 待加入偏移量、排序、提交的索引
    prepared_pindicies: Vec<u16>,

    /// InputLayout
    input_layout: ID3D11InputLayout,

    /// `Quad`、`Polygon` 通用动态顶点缓冲
    vertex_buffer: ID3D11Buffer,

    /// `Polygon` 的动态索引缓冲
    pindex_buffer: ID3D11Buffer,

    /// `QuadOnly` 的不变索引缓冲
    qindex_buffer: ID3D11Buffer,

    /// DrawCmd 合集
    draw_cmds: Vec<DrawCmd>,

    /// DrawCmd 排序索引
    cmd_idx: Vec<u32>,

    /// `draw_cmds` 变更，排序打乱
    cmd_dirted: bool,

    /// `BeginToDraw` 用于准备写入 GPU 缓存的数据
    front: BeginToDraw,
}

impl Batch2D {
    /// 最初的三角形数容量
    pub const INITIAL_PREPARE_CAPACITY_TRIANGLES: usize = 2048;

    /// 连续的四边形超过这个数字时，遇到 Polygon 时才不会将先前的 Quad 当作 
    pub const fn get_idx_initial_capacity() -> usize { Self::INITIAL_PREPARE_CAPACITY_TRIANGLES * 3 }
    pub const fn get_vert_initial_capacity() -> usize { Self::INITIAL_PREPARE_CAPACITY_TRIANGLES * 3 }
    fn generate_quad_indicies() -> Box<[u16]> {
        let mut indicies = Vec::<u16>::with_capacity(BeginToDraw::get_idx_capacity());
        for i in 0..BeginToDraw::DRAWPAGE_CAPACITY_TRIANGLES {
            let i = (i * 4) as u16;
            indicies.extend([0, 1, 3, 3 ,2, 0].map(|x| x + i));

            // 0 -- 1
            // | `. |
            // 2 -- 3
            // 0 1 3 3 2 0
        }

        indicies.into_boxed_slice()
    }
}

use windows::core::PCSTR;

const INPUT_LAYOUT_P3U2C4: [D3D11_INPUT_ELEMENT_DESC; 3] = [
    D3D11_INPUT_ELEMENT_DESC {
        SemanticName: PCSTR(b"POSITION\0".as_ptr()),
        SemanticIndex: 0,
        Format: DXGI_FORMAT_R32G32B32_FLOAT,
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
        AlignedByteOffset: 12,  // pos 占 12 字节
        InputSlotClass: D3D11_INPUT_PER_VERTEX_DATA,
        InstanceDataStepRate: 0,
    },
    D3D11_INPUT_ELEMENT_DESC {
        SemanticName: PCSTR(b"COLOR\0".as_ptr()),
        SemanticIndex: 0,
        Format: DXGI_FORMAT_R32G32B32A32_FLOAT,
        InputSlot: 0,
        AlignedByteOffset: 20,  // pos(12) + uv(8) = 20
        InputSlotClass: D3D11_INPUT_PER_VERTEX_DATA,
        InstanceDataStepRate: 0,
    },
];

impl Batch2D {
    #[must_use]
    pub fn new(gfx: Arc<super::D3D11>) -> Result<Self> {
        let qindex_buffer = d3d11_utils::create_immutable_buffer(&gfx.device, bytemuck::cast_slice(&Self::generate_quad_indicies()), D3D11_BIND_INDEX_BUFFER.0 as u32)?;
        let pindex_buffer = d3d11_utils::create_dynamic_buffer(&gfx.device, (BeginToDraw::get_idx_capacity()*std::mem::size_of::<u16>()) as u32, D3D11_BIND_INDEX_BUFFER.0 as u32)?;
        let vertex_buffer = d3d11_utils::create_dynamic_buffer(&gfx.device, (BeginToDraw::get_vert_capacity()*std::mem::size_of::<VertexP3U2C4>()) as u32, D3D11_BIND_VERTEX_BUFFER.0 as u32)?;
        let input_layout = d3d11_utils::create_input_layout(&gfx.device, &INPUT_LAYOUT_P3U2C4, include_bytes!("shaders/vs_p3u2c4_layout.cso"))?; // TODO

        Ok(Self {
            gfx,
            prepared_vertices: Vec::with_capacity(Self::get_vert_initial_capacity()),
            prepared_pindicies: Vec::with_capacity(Self::get_idx_initial_capacity()),
            qindex_buffer,
            pindex_buffer,
            vertex_buffer,
            input_layout,
            draw_cmds: Vec::with_capacity(1024),
            cmd_idx: Vec::with_capacity(1024),
            front: BeginToDraw::new(),
            cmd_dirted: false,
        })
    }

    pub fn clear(&mut self) {
        self.cmd_idx.clear();
        self.draw_cmds.clear();
        self.prepared_vertices.clear();
        self.prepared_pindicies.clear();
        self.cmd_dirted = false;
    }

    #[must_use]
    fn submit_and_draw(&self) -> Result<()> {
        if self.front.triangles == 0 { return Ok(());}
        let (verts, indicies) = self.front.get_buf_refs();

        unsafe { self.gfx.imm_context.IASetInputLayout(&self.input_layout) };
        unsafe { self.gfx.imm_context.IASetVertexBuffers(0, 1, Some(&[Some(self.vertex_buffer.clone())] as *const _), Some(&[std::mem::size_of::<VertexP3U2C4>() as u32] as *const _), Some(&[0_u32] as *const _)) };
        
        // 写入 VB
        d3d11_utils::write_buffer(&self.gfx.imm_context, &self.vertex_buffer, verts)?;

        if let Some(indicies) = indicies {
            d3d11_utils::write_buffer(&self.gfx.imm_context, &self.pindex_buffer, indicies);
            unsafe { self.gfx.imm_context.IASetIndexBuffer(Some(&self.pindex_buffer), DXGI_FORMAT_R16_UINT, 0) };
        } else {
            unsafe { self.gfx.imm_context.IASetIndexBuffer(Some(&self.qindex_buffer), DXGI_FORMAT_R16_UINT, 0) };
        }
        unsafe { self.gfx.imm_context.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);};
        unsafe { self.gfx.imm_context.DrawIndexed(self.front.triangles * 3, 0, 0) };
        Ok(())
    }

    fn prepare_state(&self, texture_id: u32, rstates_id: u32) {
        unimplemented!()
    }

    #[must_use]
    fn on_change_state(&self, texture_id: u32, rstates_id: u32) -> Result<()> {
        self.prepare_state(texture_id, rstates_id);
        self.submit_and_draw();
        Ok(())
    }

    /// 绘制后保留批处理内容
    pub fn draw_retain(&mut self) -> Result<()> {
        debug_assert_eq!(self.cmd_idx.len(), self.draw_cmds.len());
        if self.draw_cmds.len() == 0 { return Ok(()); }
        if self.cmd_dirted {
            // 稳定排序
            self.cmd_idx.sort_by_key(|i| self.draw_cmds[*i as usize].key);
            self.cmd_dirted = false;
        }
        let mut state_key = self.draw_cmds[0].key.state_key();
        for i in self.cmd_idx.iter() {
            let i = *i as usize;
            let cmd = &self.draw_cmds[i];
            let key = cmd.key;
            if key.state_key() != state_key {
                self.on_change_state(state_key.texture_id(), state_key.rstates_id());
                state_key = key.state_key();
            }
            match &cmd.content {
                DrawContent::Foregin(x) => {
                    self.on_change_state(key.texture_id(), key.rstates_id())?;
                    self.front.clear();
                    x(&self.gfx, cmd.key)
                },
                DrawContent::Polygon { start_vert, vert_count, start_index, index_count } => {
                    let start_vert = *start_vert as usize;
                    let vert_count = *vert_count as usize;
                    let start_index = *start_index as usize;
                    let index_count = *index_count as usize;
                    let vertices = &self.prepared_vertices[start_vert..start_vert+vert_count];
                    let indicies = &self.prepared_pindicies[start_index..start_index+index_count];
                    match self.front.push_polygon(vertices, indicies) {
                        PushResult::Full => {
                            self.on_change_state(key.texture_id(), key.rstates_id())?;
                            self.front.clear();
                            self.front.push_polygon(vertices, indicies);
                        }
                        PushResult::Ok => {}
                    }
                }
                DrawContent::Quad { start_vert } => {
                    let start_vert = *start_vert as usize;
                    let vertices = &self.prepared_vertices[start_vert..start_vert+4];
                    match self.front.push_quad(vertices) {
                        PushResult::Full => {
                            self.on_change_state(key.texture_id(), key.rstates_id())?;
                            self.front.clear();
                            self.front.push_quad(vertices);
                        }
                        PushResult::Ok => {}
                    }
                }
            }
        }
        self.on_change_state(state_key.texture_id(), state_key.rstates_id())?;
        self.front.clear();
        Ok(())
    }

    /// 绘制后完全清除批处理内容
    pub fn draw_flush(&mut self) {
        self.draw_retain();
        self.clear();
    }

    fn add_cmd(&mut self, cmd: DrawCmd) {
        assert!(self.draw_cmds.len() <= f32::MAX as usize);
        assert!(self.prepared_vertices.len() <= f32::MAX as usize);
        assert!(self.prepared_pindicies.len() <= f32::MAX as usize);
        self.cmd_idx.push(self.draw_cmds.len() as u32);
        self.draw_cmds.push(cmd);
    }

    /// `f` 传入的参数 `&mut [VertexP3U2C4]` 的长度一定是 `4`
    pub fn add_quad_by(&mut self, k: SortKey, f: impl Fn(&mut [VertexP3U2C4])) {
        self.prepared_vertices.reserve(4);
        let len = self.prepared_vertices.len();
        unsafe { self.prepared_vertices.set_len(len+4) };
        f(&mut self.prepared_vertices[len..len+4]);
        self.add_cmd(DrawCmd { key: k, content: DrawContent::Quad { start_vert: len as u32 }});
    }
}