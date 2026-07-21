//! 2D 批量渲染器：支持自动合批、状态排序、Basic/Advanced 模式。

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use anyhow::Result;
use windows::core::PCSTR;
use crate::platform::direct3d11::*;

use super::rstate::*;
use super::resource_manager::ResourceManager;
use super::d3d11_utils;

// ============================================================================
// 1. 安全类型定义（编译期防止 ID 混淆）
// ============================================================================

/// 纹理资源 ID
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct TextureId(pub u32);

/// 顶点着色器 ID
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct VertexShaderId(pub u32);

/// 像素着色器 ID
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct PixelShaderId(pub u32);

/// 常量缓冲区 ID
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct ConstantBufferId(pub u32);

// ============================================================================
// 2. 顶点格式
// ============================================================================

#[derive(Copy, Clone, Debug, Default, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct VertexP3U2C4 {
    pub pos: [f32; 3],
    pub uv: [f32; 2],
    pub color: [f32; 4],
}

// ============================================================================
// 3. BeginToDraw（单次 DrawCall 的缓冲区）
// ============================================================================

const DRAWPAGE_CAPACITY_TRIANGLES: usize = 2048;

#[derive(PartialEq, Eq, Debug, Clone, Copy)]
enum Indicies {
    QuadOnly,
    Polygon,
}

#[derive(PartialEq, Eq, Debug, Clone, Copy)]
enum PushResult {
    /// Don't have to reset\
    /// 无需 DrawCall & 重置
    Ok,

    /// Have to do DrawCall and reset\
    /// 需要 DrawCall & 重置
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
    const QUAD_INDICIES: [u16; 6] = [0, 1, 3, 3, 2, 0];
    const QUAD_THRESHOLD: u32 = 16;

    pub fn new() -> Self {
        let capacity = DRAWPAGE_CAPACITY_TRIANGLES * 3;
        Self {
            vertices: vec![VertexP3U2C4::default(); capacity].into_boxed_slice(),
            pindicies: vec![0; capacity].into_boxed_slice(),
            indicies: Indicies::QuadOnly,
            verts_count: 0,
            triangles: 0,
            polygoned_quads: 0,
        }
    }

    pub fn clear(&mut self) {
        self.indicies = Indicies::QuadOnly;
        self.triangles = 0;
        self.verts_count = 0;
        self.polygoned_quads = 0;
    }

    /// 接受固定数组保证编译期长度正确
    pub fn push_quad(&mut self, vertices: &[VertexP3U2C4; 4]) -> PushResult {
        if self.indicies == Indicies::Polygon {
            if self.polygoned_quads > Self::QUAD_THRESHOLD {
                return PushResult::Full;
            }
            self.polygoned_quads += 1;
            self.push_polygon(vertices, &Self::QUAD_INDICIES)
        } else {
            let input_tris = (Self::QUAD_INDICIES.len() / 3) as u32;
            if self.triangles + input_tris <= DRAWPAGE_CAPACITY_TRIANGLES as u32 {
                let start = self.verts_count as usize;
                let vert_slice = &mut self.vertices[start..start + vertices.len()];
                vert_slice.copy_from_slice(vertices);
                self.verts_count += vertices.len() as u32;
                self.triangles += input_tris;
                PushResult::Ok
            } else {
                PushResult::Full
            }
        }
    }

    pub fn push_polygon(&mut self, vertices: &[VertexP3U2C4], indicies: &[u16]) -> PushResult {
        debug_assert_eq!(
            indicies.len() % 3,
            0,
            "Index count must be multiple of 3"
        );
        // 编译期不能保证索引不越界，但运行时 debug 模式检查
        if let Some(&max_idx) = indicies.iter().max() {
            debug_assert!(
                max_idx < vertices.len() as u16,
                "Index out of bounds: max_idx={}, vertex_count={}",
                max_idx,
                vertices.len()
            );
        }
        debug_assert!(
            vertices.len() <= u16::MAX as usize,
            "Vertex count exceeds u16::MAX"
        );

        let input_tris = (indicies.len() / 3) as u32;

        if self.triangles + input_tris <= DRAWPAGE_CAPACITY_TRIANGLES as u32 {
            if self.indicies == Indicies::QuadOnly {
                // 转换已有的四边形的索引
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

            let vert_start = self.verts_count as usize;
            let vert_slice = &mut self.vertices[vert_start..vert_start + vertices.len()];
            vert_slice.copy_from_slice(vertices);

            let idx_start = self.triangles as usize * 3;
            let idx_slice = &mut self.pindicies[idx_start..idx_start + indicies.len()];
            let base = self.verts_count as u16;
            for (dst, &src) in idx_slice.iter_mut().zip(indicies.iter()) {
                *dst = src + base;
            }

            self.verts_count += vertices.len() as u32;
            self.triangles += input_tris;
            self.indicies = Indicies::Polygon;
            PushResult::Ok
        } else {
            PushResult::Full
        }
    }

    pub fn get_buf_refs(&self) -> (&[VertexP3U2C4], Option<&[u16]>) {
        let verts = &self.vertices[0..self.verts_count as usize];
        let idx = match self.indicies {
            Indicies::Polygon => Some(&self.pindicies[0..self.triangles as usize * 3]),
            Indicies::QuadOnly => None,
        };
        (verts, idx)
    }

    pub fn triangle_count(&self) -> u32 {
        self.triangles
    }
}

// ============================================================================
// 4. SortKey & DrawCmd
// ============================================================================

#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
pub struct SortKey(pub u128);

impl SortKey {
    pub fn new(z_index: u64, texture: TextureId, rstate: RState) -> Self {
        let z = z_index as u128;
        let tex = texture.0 as u128;
        let r = rstate.0 as u128;
        SortKey((z << 64) | (tex << 32) | r)
    }

    pub fn z_index(&self) -> u64 {
        (self.0 >> 64) as u64
    }

    pub fn texture_id(&self) -> TextureId {
        TextureId((self.0 >> 32) as u32)
    }

    pub fn rstate(&self) -> RState {
        RState((self.0 & 0xFFFFFFFF) as u32)
    }
}

pub trait ForeignDraw {
    /// 资源管理器 | 设备上下文 | 当前 `SortKey` | 调用（连续）`Foreign` 前最后的 `RState` \
    /// **只会在单线程中**调用。\
    /// `draw` 绝不保证初始渲染状态不变，应当自行管理。\
    /// 如果要使用之前的状态，可以尝试调用 `prepare_basic_state`.
    #[must_use]
    fn draw(&mut self, manager: &Mutex<ResourceManager>, ctx: &ID3D11DeviceContext, key: SortKey, last_rstate: RState) -> Result<()>;
}

type ForeignType = Rc<RefCell<dyn ForeignDraw>>;

#[repr(align(16))]
pub enum DrawContent {
    Quad { start_vert: u32 },
    Polygon {
        start_vert: u32,
        vert_count: u16,
        start_index: u32,
        index_count: u16,
    },
    /// 资源管理器 | 设备上下文 | 当前 `SortKey` | 调用（连续）`Foreign` 前最后的 `RState`
    Foreign(ForeignType),
}

#[repr(align(16))]
pub struct DrawCmd {
    pub key: SortKey,
    pub content: DrawContent,
}

struct Front {
    // GPU 缓冲区

    /// `P3U2C4` 的 `Layout`
    pub(super) input_layout: ID3D11InputLayout,

    /// 通用动态顶点缓冲
    pub(super) vertex_buffer: ID3D11Buffer,

    /// 多边形动态顶点缓冲
    pub(super) poly_index_buffer: ID3D11Buffer,

    /// 四边形静态顶点缓冲
    pub(super) quad_index_buffer: ID3D11Buffer,

    /// 预备数据
    pub(super) front: BeginToDraw,
}

const INPUT_LAYOUT_PUC: [D3D11_INPUT_ELEMENT_DESC; 3] = [
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
        AlignedByteOffset: 12,
        InputSlotClass: D3D11_INPUT_PER_VERTEX_DATA,
        InstanceDataStepRate: 0,
    },
    D3D11_INPUT_ELEMENT_DESC {
        SemanticName: PCSTR(b"COLOR\0".as_ptr()),
        SemanticIndex: 0,
        Format: DXGI_FORMAT_R32G32B32A32_FLOAT,
        InputSlot: 0,
        AlignedByteOffset: 20,
        InputSlotClass: D3D11_INPUT_PER_VERTEX_DATA,
        InstanceDataStepRate: 0,
    },
];

const _:() = { assert!(DRAWPAGE_CAPACITY_TRIANGLES % 2 == 0) };

fn gen_quad_indicies() -> Box<[[u16; 6]; DRAWPAGE_CAPACITY_TRIANGLES/2]> {
    let mut b = Box::new([[0_u16; 6]; DRAWPAGE_CAPACITY_TRIANGLES/2]);
    for (i, q) in b.iter_mut().enumerate() {
        *q = BeginToDraw::QUAD_INDICIES.map(|idx| idx + i as u16 * 4);
    }
    b
}

impl Front {
    fn new(device: &ID3D11Device) -> Result<Self> {
        let vertex_buffer = d3d11_utils::create_dynamic_buffer(device, (DRAWPAGE_CAPACITY_TRIANGLES * 3 * size_of::<VertexP3U2C4>()) as u32, D3D11_BIND_VERTEX_BUFFER.0 as u32)?;
        let input_layout = d3d11_utils::create_input_layout(device, &INPUT_LAYOUT_PUC, include_bytes!("shaders/p3u2c4_layout.vs.cso"))?;
        let poly_index_buffer = d3d11_utils::create_dynamic_buffer(device, (DRAWPAGE_CAPACITY_TRIANGLES * 3 * size_of::<u16>()) as u32, D3D11_BIND_INDEX_BUFFER.0 as u32)?;
        let quad_index_buffer = d3d11_utils::create_immutable_buffer(device, bytemuck::cast_slice(gen_quad_indicies().as_slice()), D3D11_BIND_INDEX_BUFFER.0 as u32)?;
        let front = BeginToDraw::new();
        Ok(Self {
            vertex_buffer,
            input_layout,
            poly_index_buffer,
            quad_index_buffer,
            front,
        })
    }
}

/// 设置为 `pub` 方便闭包调用
pub fn prepare_basic_state(manager: &Arc<Mutex<ResourceManager>>, ctx: &ID3D11DeviceContext, rstate: RState) {
    let mut manager = manager.lock().unwrap();
    unsafe {
        let blend = manager.get_basic_blend(rstate.blend_idx());
        ctx.OMSetBlendState(Some(blend), None, 0xFFFFFFFF);
        let sampler = manager.get_basic_sampler(rstate.sampler_idx()).clone();
        ctx.PSSetSamplers(0, Some(&[Some(sampler)]));
        let raster = manager.get_basic_rasterizer(rstate.raster_idx());
        ctx.RSSetState(Some(raster));

        // 深度/模板：合并为一个索引
        // bit0-1: 模板模式, bit2: 深度测试, bit3: 深度写入
        let ds_idx = rstate.stencil_idx()
            | ((rstate.depth_test() as u8) << 2)
            | ((rstate.depth_write() as u8) << 3);
        let ds = manager.get_basic_depth_stencil(ds_idx);
        ctx.OMSetDepthStencilState(Some(ds), 0);
    }
}

// ============================================================================
// 5. Batch2D 主结构
// ============================================================================

pub struct Batch2D {
    manager: Arc<Mutex<ResourceManager>>,
    device: ID3D11Device,

    // 准备好的顶点/索引数据
    prepared_vertices: Vec<VertexP3U2C4>,
    prepared_pindicies: Vec<u16>,

    // 命令列表
    draw_cmds: Vec<DrawCmd>,
    cmd_idx: Vec<u32>,
    cmd_dirted: bool,

    // Front
    front: Front,
}

impl Batch2D {
    const INITIAL_CAPACITY_TRIANGLES: usize = 2048;

    pub fn new(
        device: ID3D11Device,
        manager: Arc<Mutex<ResourceManager>>,
    ) -> Result<Self> {
        let capacity = Self::INITIAL_CAPACITY_TRIANGLES * 3;
        Ok(Self {
            manager,
            front: Front::new(&device)?,
            device,
            prepared_vertices: Vec::with_capacity(capacity),
            prepared_pindicies: Vec::with_capacity(capacity),
            draw_cmds: Vec::with_capacity(1024),
            cmd_idx: Vec::with_capacity(1024),
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

    

    // ---------- 提交与绘制 ----------


    #[must_use = "Result should be used"]
    fn submit_and_draw(front: &mut Front, ctx: &ID3D11DeviceContext) -> Result<()> {
        if front.front.triangle_count() == 0 {
            return Ok(());
        }

        let (verts, indicies) = front.front.get_buf_refs();

        unsafe {
            ctx.IASetInputLayout(&front.input_layout);
            ctx.IASetVertexBuffers(
                0,
                1,
                Some([Some(front.vertex_buffer.clone())].as_ptr()),
                Some([std::mem::size_of::<VertexP3U2C4>() as u32].as_ptr()),
                Some([0_u32].as_ptr()),
            );
        }

        // 写入顶点数据
        d3d11_utils::write_buffer(ctx, &front.vertex_buffer, verts)?;

        if let Some(indicies) = indicies {
            d3d11_utils::write_buffer(ctx, &front.poly_index_buffer, indicies)?;
            unsafe {
                ctx.IASetIndexBuffer(Some(&front.poly_index_buffer), DXGI_FORMAT_R16_UINT, 0);
            }
        } else {
            unsafe {
                ctx.IASetIndexBuffer(Some(&front.quad_index_buffer), DXGI_FORMAT_R16_UINT, 0);
            }
        }

        unsafe {
            ctx.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
            ctx.DrawIndexed(front.front.triangle_count() * 3, 0, 0);
        }

        front.front.clear();
        Ok(())
    }

    pub fn draw_retain(&mut self, ctx: &ID3D11DeviceContext) -> Result<()> {
        if self.draw_cmds.is_empty() {
            return Ok(());
        }

        // 排序
        if self.cmd_dirted {
            self.cmd_idx
                .sort_unstable_by_key(|&i| self.draw_cmds[i as usize].key);
            self.cmd_dirted = false;
        }

        let mut current_rstate = self.draw_cmds[self.cmd_idx[0] as usize].key.rstate();
        if current_rstate.is_basic() {
            prepare_basic_state(&self.manager, ctx, current_rstate);
        } else {
            self.manager.lock().unwrap().bind_advanced_state(ctx, current_rstate);
        }

        let mut last_foreign = false;


        for &idx in &self.cmd_idx {
            let key = self.draw_cmds[idx as usize].key;
            let rstate = key.rstate();

            let content = &self.draw_cmds[idx as usize].content;
            if last_foreign { if let DrawContent::Foreign(_) = content {} else {
                if rstate.is_basic() {
                    prepare_basic_state(&self.manager, ctx, rstate);
                } else {
                    self.manager.lock().unwrap().bind_advanced_state(ctx, rstate);
                }
                current_rstate = rstate;
                last_foreign = false;
            }} else if rstate != current_rstate {
                // 提交当前批次
                Self::submit_and_draw(&mut self.front, ctx)?;

                // 切换状态
                if rstate.is_basic() {
                    prepare_basic_state(&self.manager, ctx, rstate);
                } else {
                    self.manager.lock().unwrap().bind_advanced_state(ctx, rstate);
                }
                current_rstate = rstate;
            }

            // 逐个提取内容数据，再调用需要 &mut self 的方法
            match content {
                DrawContent::Quad { start_vert } => {
                    let start = *start_vert as usize;
                    // 构造固定数组，编译期安全
                    let vertices = [
                        self.prepared_vertices[start],
                        self.prepared_vertices[start + 1],
                        self.prepared_vertices[start + 2],
                        self.prepared_vertices[start + 3],
                    ];
                    if let PushResult::Full = self.front.front.push_quad(&vertices) {
                        Self::submit_and_draw(&mut self.front, ctx)?;
                        self.front.front.push_quad(&vertices);
                    }
                }
                DrawContent::Polygon {
                    start_vert,
                    vert_count,
                    start_index,
                    index_count,
                } => {
                    let s_vert = *start_vert as usize;
                    let v_cnt = *vert_count as usize;
                    let s_idx = *start_index as usize;
                    let i_cnt = *index_count as usize;
                    let vertices =
                        &self.prepared_vertices[s_vert..s_vert + v_cnt];
                    let indicies =
                        &self.prepared_pindicies[s_idx..s_idx + i_cnt];
                    if let PushResult::Full = self.front.front.push_polygon(vertices, indicies) {
                        Self::submit_and_draw(&mut self.front, ctx)?;
                        self.front.front.push_polygon(&vertices, &indicies);
                    }
                }
                DrawContent::Foreign(callback) => {
                    Self::submit_and_draw(&mut self.front, ctx)?;
                    callback.borrow_mut().draw(&self.manager, ctx, key, current_rstate)?;
                    last_foreign = true;
                }
            }
        }

        // 提交最后一批
        Self::submit_and_draw(&mut self.front, ctx)?;
        Ok(())
    }

    pub fn draw_flush(&mut self, ctx: &ID3D11DeviceContext) -> Result<()> {
        let result = self.draw_retain(ctx);
        self.clear();
        result
    }
}

// 编译期检查：确保 DrawCmd 大小 <= 64 字节
const _: () = {
    assert!(
        std::mem::size_of::<DrawCmd>() <= 64,
        "DrawCmd size must be <= 64 bytes for SIMD efficiency"
    );
};

impl Batch2D {
    // ---------- 添加绘制命令（编译期安全） ----------

    /// 添加四边形，闭包接收固定长度数组 [VertexP3U2C4; 4]
    pub fn add_quad_by(&mut self, key: SortKey, f: impl FnOnce(&mut [VertexP3U2C4; 4])) {
        let len = self.prepared_vertices.len();
        self.prepared_vertices.resize(len + 4, VertexP3U2C4::default());
        // 安全：长度固定为 4，try_into 不会失败
        let arr: &mut [VertexP3U2C4; 4] = (&mut self.prepared_vertices[len..len + 4])
            .try_into()
            .unwrap();
        f(arr);
        self.add_cmd(DrawCmd {
            key,
            content: DrawContent::Quad { start_vert: len as u32 },
        });
    }

    /// 添加多边形，debug 模式下会检查索引是否越界
    pub fn add_polygon(
        &mut self,
        key: SortKey,
        vertices: &[VertexP3U2C4],
        triangles: &[[u16; 3]],
    ) {
        #[cfg(debug_assertions)]
        {
            for tri in triangles {
                for &idx in tri {
                    debug_assert!(idx < vertices.len() as u16, "Index {} out of bounds (vertex count {})", idx, vertices.len());
                }
            }
        }
        debug_assert!(
            vertices.len() <= u16::MAX as usize,
            "Vertex count exceeds u16::MAX"
        );

        let vert_start = self.prepared_vertices.len();
        self.prepared_vertices.extend_from_slice(vertices);
        let idx_start = self.prepared_pindicies.len();
        self.prepared_pindicies.extend_from_slice(bytemuck::cast_slice(triangles));
        self.add_cmd(DrawCmd {
            key,
            content: DrawContent::Polygon {
                start_vert: vert_start as u32,
                vert_count: vertices.len() as u16,
                start_index: idx_start as u32,
                index_count: triangles.len() as u16 * 3,
            },
        });
    }

    /// 添加自定义绘制回调
    pub fn add_foreign(
        &mut self,
        key: SortKey,
        callback: ForeignType,
    ) {
        self.add_cmd(DrawCmd {
            key,
            content: DrawContent::Foreign(callback),
        });
    }

    fn add_cmd(&mut self, cmd: DrawCmd) {
        self.cmd_idx.push(self.draw_cmds.len() as u32);
        self.draw_cmds.push(cmd);
        self.cmd_dirted = true;
    }
}