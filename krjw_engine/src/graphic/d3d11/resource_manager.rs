//! 资源管理器：负责纹理、Shader、状态对象的创建、缓存和绑定。
//!
//! 使用 `ResourcePool` 封装 HashMap，提供字符串 ID 到整数 ID 的映射，
//! 以及整数 ID 到 D3D11 对象的缓存。

use std::collections::HashMap;
use std::sync::Arc;
use anyhow::Result;
use glam::Vec2;
use windows::Win32::Graphics::{Direct3D::*, Direct3D11::*, Dxgi::Common::*};

use crate::{TextureInfo};

use super::d3d11_utils::{self, create_texture_2d, create_srv};
use super::rstate::*;

// ============================================================================
// 1. 资源池封装（HashMap<String, u32> + HashMap<u32, T>）
// ============================================================================

/// 通用资源池：管理字符串名称到整数 ID 的映射，以及整数 ID 到实际对象的缓存。
#[derive(Clone)]
pub struct ResourcePool<T> {
    name_to_id: HashMap<String, u32>,
    id_to_obj: HashMap<u32, T>,
    next_id: u32,
}

impl<T> ResourcePool<T> {
    pub fn new() -> Self {
        Self {
            name_to_id: HashMap::new(),
            id_to_obj: HashMap::new(),
            next_id: 1, // 0 保留给默认/空资源
        }
    }

    /// 通过名称获取 ID，若不存在则返回 None
    pub fn get_id(&self, name: &str) -> Option<u32> {
        self.name_to_id.get(name).copied()
    }

    /// 通过 ID 获取对象引用
    pub fn get(&self, id: u32) -> Option<&T> {
        self.id_to_obj.get(&id)
    }

    /// 插入新资源，返回分配的 ID
    pub fn insert(&mut self, name: &str, obj: T) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        self.name_to_id.insert(name.to_string(), id);
        self.id_to_obj.insert(id, obj);
        id
    }

    /// 检查是否包含某名称
    pub fn contains_name(&self, name: &str) -> bool {
        self.name_to_id.contains_key(name)
    }

    /// 获取所有名称（用于调试）
    pub fn names(&self) -> impl Iterator<Item = &String> {
        self.name_to_id.keys()
    }
}

impl<T> Default for ResourcePool<T> {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// 2. 渲染状态组合描述符（Advanced 模式）
// ============================================================================

/// Advanced 模式的状态描述符，支持自定义 Shader、额外纹理、CB 等。
#[derive(Hash, PartialEq, Eq, Clone, Copy)]
pub struct AdvancedStateDesc {
    pub vs_id: u32,
    pub ps_id: u32,
    pub main_sampler_id: u32,
    pub blend_id: u32,
    pub rasterizer_id: u32,
    pub depth_stencil_id: u32,
    pub extra_id: u32, // 指向 ExtraResourceDesc
}

/// 额外资源描述：纹理槽 + 常量缓冲槽
#[derive(Hash, PartialEq, Eq, Clone, Copy)]
pub struct ExtraResourceDesc {
    /// 槽 1-8，0 表示未设置
    pub textures: [u32; 8],   
    /// 槽 1-4，0 表示未设置
    pub cbuffers: [u32; 4],   
}

impl Default for ExtraResourceDesc {
    fn default() -> Self {
        Self {
            textures: [0; 8],
            cbuffers: [0; 4],
        }
    }
}

// ============================================================================
// 3. ResourceManager 主结构
// ============================================================================

/// 资源管理器
pub struct ResourceManager {
    device: ID3D11Device,

    // 资源池
    pub textures: ResourcePool<Arc<TextureInfo>>,
    pub vertex_shaders: ResourcePool<ID3D11VertexShader>,
    pub pixel_shaders: ResourcePool<ID3D11PixelShader>,
    pub samplers: ResourcePool<ID3D11SamplerState>,
    pub blend_states: ResourcePool<ID3D11BlendState>,
    pub rasterizer_states: ResourcePool<ID3D11RasterizerState>,
    pub depth_stencil_states: ResourcePool<ID3D11DepthStencilState>,
    pub constant_buffers: ResourcePool<ID3D11Buffer>,

    // Advanced 状态缓存
    extra_cache: HashMap<ExtraResourceDesc, u32>,
    extra_reverse: HashMap<u32, ExtraResourceDesc>,
    advanced_cache: HashMap<AdvancedStateDesc, u32>,
    advanced_reverse: HashMap<u32, AdvancedStateDesc>,

    // Basic 状态懒加载缓存（按索引缓存 D3D11 对象）
    basic_blend_states: [Option<ID3D11BlendState>; 16],
    basic_sampler_states: [Option<ID3D11SamplerState>; 16],
    basic_rasterizer_states: [Option<ID3D11RasterizerState>; 16],
    basic_depth_stencil_states: [Option<ID3D11DepthStencilState>; 16],

    next_advanced_id: u32,

    /// 默认 1×1 白色纹理的 ID（在 textures pool 中）
    pub white_tex_id: u32,
}

impl ResourceManager {
    pub fn new(device: ID3D11Device) -> Self {
        let mut this = Self {
            device,
            textures: ResourcePool::new(),
            vertex_shaders: ResourcePool::new(),
            pixel_shaders: ResourcePool::new(),
            samplers: ResourcePool::new(),
            blend_states: ResourcePool::new(),
            rasterizer_states: ResourcePool::new(),
            depth_stencil_states: ResourcePool::new(),
            constant_buffers: ResourcePool::new(),
            extra_cache: HashMap::new(),
            extra_reverse: HashMap::new(),
            advanced_cache: HashMap::new(),
            advanced_reverse: HashMap::new(),
            basic_blend_states: [None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None],
            basic_sampler_states: [None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None],
            basic_rasterizer_states: [None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None],
            basic_depth_stencil_states: [None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None],
            next_advanced_id: 1,
            white_tex_id: 0,
        };

        // 创建默认的 1×1 白色 RGBA 纹理
        let white_pixel: [u8; 4] = [255; 4];
        let tex = create_texture_2d(
            &this.device,
            1,
            1,
            DXGI_FORMAT_R8G8B8A8_UNORM,
            D3D11_BIND_SHADER_RESOURCE.0 as u32,
            D3D11_USAGE_IMMUTABLE,
            0,
            Some((&white_pixel, 4)),
        )
        .expect("Failed to create default white texture");
        let srv = create_srv(&this.device, &tex, DXGI_FORMAT_R8G8B8A8_UNORM)
            .expect("Failed to create SRV for default white texture");
        let tex_info = Arc::new(TextureInfo {
            texture: tex,
            srv,
            width: 1,
            height: 1,
            size_inv: Vec2::ONE,
            format: DXGI_FORMAT_R8G8B8A8_UNORM,
        });
        this.white_tex_id = this.textures.insert("_white", tex_info);

        this
    }

    // ---------- Basic 状态懒加载 ----------
    pub fn get_basic_blend(&mut self, idx: u8) -> &ID3D11BlendState {
        assert!(idx < 16);
        let idx = idx as usize;
        if self.basic_blend_states[idx].is_none() {
            let desc = Self::build_blend_desc(idx as u8);
            let mut state: Option<ID3D11BlendState> = None;
            unsafe {
                self.device
                    .CreateBlendState(&desc, Some(&mut state))
                    .expect("Failed to create blend state")
            };
            self.basic_blend_states[idx] = state;
        }
        self.basic_blend_states[idx].as_ref().unwrap()
    }

    pub fn get_basic_sampler(&mut self, idx: u8) -> &ID3D11SamplerState {
        assert!(idx < 16);
        let idx = idx as usize;
        if self.basic_sampler_states[idx].is_none() {
            let desc = Self::build_sampler_desc(idx as u8);
            let mut state: Option<ID3D11SamplerState> = None;
            unsafe {
                self.device
                    .CreateSamplerState(&desc, Some(&mut state))
                    .expect("Failed to create sampler state")
            };
            self.basic_sampler_states[idx] = state;
        }
        self.basic_sampler_states[idx].as_ref().unwrap()
    }

    pub fn get_basic_rasterizer(&mut self, idx: u8) -> &ID3D11RasterizerState {
        assert!(idx < 16);
        let idx = idx as usize;
        if self.basic_rasterizer_states[idx].is_none() {
            let desc = Self::build_rasterizer_desc(idx as u8);
            let mut state: Option<ID3D11RasterizerState> = None;
            unsafe {
                self.device
                    .CreateRasterizerState(&desc, Some(&mut state))
                    .expect("Failed to create rasterizer state")
            };
            self.basic_rasterizer_states[idx] = state;
        }
        self.basic_rasterizer_states[idx].as_ref().unwrap()
    }

    pub fn get_basic_depth_stencil(&mut self, idx: u8) -> &ID3D11DepthStencilState {
        assert!(idx < 16);
        let idx = idx as usize;
        if self.basic_depth_stencil_states[idx].is_none() {
            let desc = Self::build_depth_stencil_desc(idx as u8);
            let mut state: Option<ID3D11DepthStencilState> = None;
            unsafe {
                self.device
                    .CreateDepthStencilState(&desc, Some(&mut state))
                    .expect("Failed to create depth stencil state")
            };
            self.basic_depth_stencil_states[idx] = state;
        }
        self.basic_depth_stencil_states[idx].as_ref().unwrap()
    }

    // ---------- Basic 状态描述构建（由枚举索引驱动） ----------
    fn build_blend_desc(idx: u8) -> D3D11_BLEND_DESC {
        use windows::Win32::Graphics::Direct3D11::*;
        let mut desc: D3D11_BLEND_DESC = unsafe { std::mem::zeroed() };
        desc.AlphaToCoverageEnable = false.into();
        desc.IndependentBlendEnable = false.into();
        let rt = &mut desc.RenderTarget[0];
        rt.RenderTargetWriteMask = D3D11_COLOR_WRITE_ENABLE_ALL.0 as u8;

        match idx {
            0 => { // Normal: SrcAlpha, InvSrcAlpha
                rt.SrcBlend = D3D11_BLEND_SRC_ALPHA;
                rt.DestBlend = D3D11_BLEND_INV_SRC_ALPHA;
                rt.BlendOp = D3D11_BLEND_OP_ADD;
                rt.SrcBlendAlpha = D3D11_BLEND_SRC_ALPHA;
                rt.DestBlendAlpha = D3D11_BLEND_INV_SRC_ALPHA;
                rt.BlendOpAlpha = D3D11_BLEND_OP_ADD;
                rt.BlendEnable = true.into();
            }
            1 => { // Additive: One, One
                rt.SrcBlend = D3D11_BLEND_ONE;
                rt.DestBlend = D3D11_BLEND_ONE;
                rt.BlendOp = D3D11_BLEND_OP_ADD;
                rt.SrcBlendAlpha = D3D11_BLEND_ONE;
                rt.DestBlendAlpha = D3D11_BLEND_ONE;
                rt.BlendOpAlpha = D3D11_BLEND_OP_ADD;
                rt.BlendEnable = true.into();
            }
            2 => { // Multiply: Zero, SrcColor
                rt.SrcBlend = D3D11_BLEND_ZERO;
                rt.DestBlend = D3D11_BLEND_SRC_COLOR;
                rt.BlendOp = D3D11_BLEND_OP_ADD;
                rt.SrcBlendAlpha = D3D11_BLEND_ZERO;
                rt.DestBlendAlpha = D3D11_BLEND_SRC_COLOR;
                rt.BlendOpAlpha = D3D11_BLEND_OP_ADD;
                rt.BlendEnable = true.into();
            }
            3 => { // Premultiplied: One, InvSrcAlpha
                rt.SrcBlend = D3D11_BLEND_ONE;
                rt.DestBlend = D3D11_BLEND_INV_SRC_ALPHA;
                rt.BlendOp = D3D11_BLEND_OP_ADD;
                rt.SrcBlendAlpha = D3D11_BLEND_ONE;
                rt.DestBlendAlpha = D3D11_BLEND_INV_SRC_ALPHA;
                rt.BlendOpAlpha = D3D11_BLEND_OP_ADD;
                rt.BlendEnable = true.into();
            }
            4 => { // Subtract: SrcAlpha, InvSrcAlpha, SUBTRACT
                rt.SrcBlend = D3D11_BLEND_SRC_ALPHA;
                rt.DestBlend = D3D11_BLEND_INV_SRC_ALPHA;
                rt.BlendOp = D3D11_BLEND_OP_SUBTRACT;
                rt.SrcBlendAlpha = D3D11_BLEND_SRC_ALPHA;
                rt.DestBlendAlpha = D3D11_BLEND_INV_SRC_ALPHA;
                rt.BlendOpAlpha = D3D11_BLEND_OP_SUBTRACT;
                rt.BlendEnable = true.into();
            }
            5 => { // ReverseSubtract
                rt.SrcBlend = D3D11_BLEND_SRC_ALPHA;
                rt.DestBlend = D3D11_BLEND_INV_SRC_ALPHA;
                rt.BlendOp = D3D11_BLEND_OP_REV_SUBTRACT;
                rt.SrcBlendAlpha = D3D11_BLEND_SRC_ALPHA;
                rt.DestBlendAlpha = D3D11_BLEND_INV_SRC_ALPHA;
                rt.BlendOpAlpha = D3D11_BLEND_OP_REV_SUBTRACT;
                rt.BlendEnable = true.into();
            }
            6 => { // Min
                rt.SrcBlend = D3D11_BLEND_ONE;
                rt.DestBlend = D3D11_BLEND_ONE;
                rt.BlendOp = D3D11_BLEND_OP_MIN;
                rt.SrcBlendAlpha = D3D11_BLEND_ONE;
                rt.DestBlendAlpha = D3D11_BLEND_ONE;
                rt.BlendOpAlpha = D3D11_BLEND_OP_MIN;
                rt.BlendEnable = true.into();
            }
            7 => { // Max
                rt.SrcBlend = D3D11_BLEND_ONE;
                rt.DestBlend = D3D11_BLEND_ONE;
                rt.BlendOp = D3D11_BLEND_OP_MAX;
                rt.SrcBlendAlpha = D3D11_BLEND_ONE;
                rt.DestBlendAlpha = D3D11_BLEND_ONE;
                rt.BlendOpAlpha = D3D11_BLEND_OP_MAX;
                rt.BlendEnable = true.into();
            }
            8 => { // Opaque: One, Zero
                rt.SrcBlend = D3D11_BLEND_ONE;
                rt.DestBlend = D3D11_BLEND_ZERO;
                rt.BlendOp = D3D11_BLEND_OP_ADD;
                rt.SrcBlendAlpha = D3D11_BLEND_ONE;
                rt.DestBlendAlpha = D3D11_BLEND_ZERO;
                rt.BlendOpAlpha = D3D11_BLEND_OP_ADD;
                rt.BlendEnable = false.into();
            }
            9 => { // Invert: InvDstColor, Zero
                rt.SrcBlend = D3D11_BLEND_INV_DEST_COLOR;
                rt.DestBlend = D3D11_BLEND_ZERO;
                rt.BlendOp = D3D11_BLEND_OP_ADD;
                rt.SrcBlendAlpha = D3D11_BLEND_INV_DEST_COLOR;
                rt.DestBlendAlpha = D3D11_BLEND_ZERO;
                rt.BlendOpAlpha = D3D11_BLEND_OP_ADD;
                rt.BlendEnable = true.into();
            }
            _ => unimplemented!("Custom blend mode {}", idx),
        }
        desc
    }

    fn build_sampler_desc(idx: u8) -> D3D11_SAMPLER_DESC {
        use windows::Win32::Graphics::Direct3D11::*;
        let mut desc: D3D11_SAMPLER_DESC = unsafe { std::mem::zeroed() };
        desc.AddressU = D3D11_TEXTURE_ADDRESS_CLAMP;
        desc.AddressV = D3D11_TEXTURE_ADDRESS_CLAMP;
        desc.AddressW = D3D11_TEXTURE_ADDRESS_CLAMP;
        desc.MinLOD = 0.0;
        desc.MaxLOD = f32::MAX;
        desc.MipLODBias = 0.0;
        desc.MaxAnisotropy = 1;
        desc.ComparisonFunc = D3D11_COMPARISON_NEVER;

        match idx {
            0 => { // PointClamp
                desc.Filter = D3D11_FILTER_MIN_MAG_MIP_POINT;
            }
            1 => { // PointWrap
                desc.Filter = D3D11_FILTER_MIN_MAG_MIP_POINT;
                desc.AddressU = D3D11_TEXTURE_ADDRESS_WRAP;
                desc.AddressV = D3D11_TEXTURE_ADDRESS_WRAP;
                desc.AddressW = D3D11_TEXTURE_ADDRESS_WRAP;
            }
            2 => { // LinearClamp
                desc.Filter = D3D11_FILTER_MIN_MAG_MIP_LINEAR;
            }
            3 => { // LinearWrap
                desc.Filter = D3D11_FILTER_MIN_MAG_MIP_LINEAR;
                desc.AddressU = D3D11_TEXTURE_ADDRESS_WRAP;
                desc.AddressV = D3D11_TEXTURE_ADDRESS_WRAP;
                desc.AddressW = D3D11_TEXTURE_ADDRESS_WRAP;
            }
            4 => { // AnisoClamp
                desc.Filter = D3D11_FILTER_ANISOTROPIC;
                desc.MaxAnisotropy = 4;
            }
            5 => { // AnisoWrap
                desc.Filter = D3D11_FILTER_ANISOTROPIC;
                desc.MaxAnisotropy = 4;
                desc.AddressU = D3D11_TEXTURE_ADDRESS_WRAP;
                desc.AddressV = D3D11_TEXTURE_ADDRESS_WRAP;
                desc.AddressW = D3D11_TEXTURE_ADDRESS_WRAP;
            }
            _ => unimplemented!("Custom sampler mode {}", idx),
        }
        desc
    }

    fn build_rasterizer_desc(idx: u8) -> D3D11_RASTERIZER_DESC {
        use windows::Win32::Graphics::Direct3D11::*;
        let mut desc: D3D11_RASTERIZER_DESC = unsafe { std::mem::zeroed() };
        desc.FillMode = D3D11_FILL_SOLID;
        desc.CullMode = D3D11_CULL_NONE;
        desc.FrontCounterClockwise = false.into();
        desc.DepthBias = 0;
        desc.DepthBiasClamp = 0.0;
        desc.SlopeScaledDepthBias = 0.0;
        desc.DepthClipEnable = true.into();
        desc.ScissorEnable = false.into();
        desc.MultisampleEnable = false.into();
        desc.AntialiasedLineEnable = false.into();

        match idx {
            0 => { // CullNone
                desc.CullMode = D3D11_CULL_NONE;
            }
            1 => { // CullCW
                desc.CullMode = D3D11_CULL_FRONT;
            }
            2 => { // CullCCW
                desc.CullMode = D3D11_CULL_BACK;
            }
            3 => { // Wireframe
                desc.FillMode = D3D11_FILL_WIREFRAME;
            }
            _ => unimplemented!("Custom rasterizer mode {}", idx),
        }
        desc
    }

    fn build_depth_stencil_desc(idx: u8) -> D3D11_DEPTH_STENCIL_DESC {
        use windows::Win32::Graphics::Direct3D11::*;
        let mut desc: D3D11_DEPTH_STENCIL_DESC = unsafe { std::mem::zeroed() };
        desc.DepthEnable = false.into();
        desc.DepthWriteMask = D3D11_DEPTH_WRITE_MASK_ZERO;
        desc.DepthFunc = D3D11_COMPARISON_ALWAYS;
        desc.StencilEnable = false.into();
        desc.StencilReadMask = 0xFF;
        desc.StencilWriteMask = 0xFF;

        // 解析标志位（bit13=深度测试, bit14=深度写入, bit15-16=模板模式）
        let depth_test = (idx & 0x1) != 0;
        let depth_write = (idx & 0x2) != 0;
        let stencil_mode = (idx >> 2) & 0x3;

        if depth_test {
            desc.DepthEnable = true.into();
            desc.DepthFunc = D3D11_COMPARISON_LESS_EQUAL;
            desc.DepthWriteMask = if depth_write {
                D3D11_DEPTH_WRITE_MASK_ALL
            } else {
                D3D11_DEPTH_WRITE_MASK_ZERO
            };
        }

        match stencil_mode {
            0 => { // Disabled
                desc.StencilEnable = false.into();
            }
            1 => { // Write (REPLACE 1)
                desc.StencilEnable = true.into();
                desc.FrontFace.StencilFunc = D3D11_COMPARISON_ALWAYS;
                desc.FrontFace.StencilPassOp = D3D11_STENCIL_OP_REPLACE;
                desc.FrontFace.StencilFailOp = D3D11_STENCIL_OP_REPLACE;
                desc.FrontFace.StencilDepthFailOp = D3D11_STENCIL_OP_REPLACE;
                desc.BackFace = desc.FrontFace;
            }
            2 => { // Read (EQUAL 1)
                desc.StencilEnable = true.into();
                desc.StencilReadMask = 0x01;
                desc.StencilWriteMask = 0x00;
                desc.FrontFace.StencilFunc = D3D11_COMPARISON_EQUAL;
                desc.FrontFace.StencilPassOp = D3D11_STENCIL_OP_KEEP;
                desc.FrontFace.StencilFailOp = D3D11_STENCIL_OP_KEEP;
                desc.FrontFace.StencilDepthFailOp = D3D11_STENCIL_OP_KEEP;
                desc.BackFace = desc.FrontFace;
            }
            3 => { // Invert (XOR)
                desc.StencilEnable = true.into();
                desc.FrontFace.StencilFunc = D3D11_COMPARISON_ALWAYS;
                desc.FrontFace.StencilPassOp = D3D11_STENCIL_OP_INVERT;
                desc.FrontFace.StencilFailOp = D3D11_STENCIL_OP_INVERT;
                desc.FrontFace.StencilDepthFailOp = D3D11_STENCIL_OP_INVERT;
                desc.BackFace = desc.FrontFace;
            }
            _ => unreachable!(),
        }
        desc
    }

    // ---------- Advanced 状态管理 ----------
    pub fn get_or_create_extra_id(&mut self, desc: ExtraResourceDesc) -> u32 {
        if let Some(&id) = self.extra_cache.get(&desc) {
            return id;
        }
        let id = self.next_advanced_id;
        self.next_advanced_id += 1;
        self.extra_cache.insert(desc, id);
        self.extra_reverse.insert(id, desc);
        id
    }

    pub fn get_or_create_advanced_id(&mut self, desc: AdvancedStateDesc) -> u32 {
        if let Some(&id) = self.advanced_cache.get(&desc) {
            return id;
        }
        let id = self.next_advanced_id;
        self.next_advanced_id += 1;
        self.advanced_cache.insert(desc, id);
        self.advanced_reverse.insert(id, desc);
        id
    }

    pub fn get_advanced_desc(&self, id: u32) -> Option<&AdvancedStateDesc> {
        self.advanced_reverse.get(&id)
    }

    pub fn get_extra_desc(&self, id: u32) -> Option<&ExtraResourceDesc> {
        self.extra_reverse.get(&id)
    }

    // ---------- 纹理查询便利方法 ----------

    /// 通过名称获取纹理，找不到则回退到 `_white`
    pub fn get_texture_or_white(&self, name: &str) -> (u32, &Arc<TextureInfo>) {
        let id = self.textures.get_id(name).unwrap_or(self.white_tex_id);
        let tex = self.textures.get(id).unwrap();
        (id, tex)
    }

    /// 通过 ID 获取纹理引用
    pub fn get_texture_by_id(&self, id: u32) -> Option<&Arc<TextureInfo>> {
        self.textures.get(id)
    }

    /// 通过 ID 获取纹理，找不到则回退到 `_white`
    pub fn get_texture_by_id_or_white(&self, id: u32) -> &Arc<TextureInfo> {
        self.textures.get(id).unwrap_or_else(|| self.textures.get(self.white_tex_id).unwrap())
    }

    // ---------- 绑定 Advanced 状态到上下文 ----------
    pub fn bind_advanced_state(
        &self,
        ctx: &ID3D11DeviceContext,
        rstate: RState,
    ) {
        if !rstate.is_advanced() {
            return;
        }
        let id = rstate.advanced_id();
        let desc = match self.advanced_reverse.get(&id) {
            Some(d) => d,
            None => return,
        };

        unsafe {
            // 绑定着色器
            if let Some(vs) = self.vertex_shaders.get(desc.vs_id) {
                ctx.VSSetShader(Some(vs), None);
            }
            if let Some(ps) = self.pixel_shaders.get(desc.ps_id) {
                ctx.PSSetShader(Some(ps), None);
            }

            // 绑定额外纹理和 CB
            if let Some(extra) = self.extra_reverse.get(&desc.extra_id) {
                for (slot, &tex_id) in extra.textures.iter().enumerate() {
                    if tex_id != 0 {
                        if let Some(tex) = self.textures.get(tex_id) {
                            ctx.PSSetShaderResources(slot as u32, Some(&[Some(tex.srv.clone())]));
                        }
                    }
                }
                for (slot, &cb_id) in extra.cbuffers.iter().enumerate() {
                    if cb_id != 0 {
                        if let Some(buffer) = self.constant_buffers.get(cb_id) {
                            ctx.PSSetConstantBuffers(slot as u32, Some(&[Some((buffer.clone()))]));
                        }
                    }
                }
            }

            // 采样器
            if let Some(sampler) = self.samplers.get(desc.main_sampler_id) {
                ctx.PSSetSamplers(0, Some(&[Some(sampler.clone())]));
            }

            // 混合、光栅化、深度
            if let Some(blend) = self.blend_states.get(desc.blend_id) {
                ctx.OMSetBlendState(Some(blend), None, 0xFFFFFFFF);
            }
            if let Some(raster) = self.rasterizer_states.get(desc.rasterizer_id) {
                ctx.RSSetState(Some(raster));
            }
            if let Some(ds) = self.depth_stencil_states.get(desc.depth_stencil_id) {
                ctx.OMSetDepthStencilState(Some(ds), 0);
            }
        }
    }
}