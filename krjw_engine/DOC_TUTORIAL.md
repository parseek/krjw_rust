# KRJW_Engine 完整文档与教程

> 基于 Direct3D 11、`winit`、`glam` 和 `kira` 的可复用 2D 精灵引擎。  
> **作者的话**：因为作者懒得造轮子，所以大部分代码是 Vibe 出来的。  

---

## 目录

1. [概述与架构](#1-概述与架构)
2. [核心概念](#2-核心概念)
   - 2.1 [双线程架构解析](#21-双线程架构解析)
   - 2.2 [批渲染原理](#22-批渲染原理)
   - 2.3 [Pipeline 模式与排序](#23-pipeline-模式与排序)
   - 2.4 [边缘检测的位运算原理](#24-边缘检测的位运算原理)
   - 2.5 [MPSC 通道时序](#25-mpsc-通道时序)
3. [快速开始](#3-快速开始)
   - 3.1 [创建 Workspace 和项目](#31-创建-workspace-和项目)
   - 3.2 [添加依赖](#32-添加依赖)
   - 3.3 [最小入口 main.rs](#33-最小入口-mainrs)
   - 3.4 [应用骨架 app.rs](#34-应用骨架-apprs)
   - 3.5 [添加窗口属性定制](#35-添加窗口属性定制)
4. [模块详解](#4-模块详解)
   - 4.1 [EngineHandler](#41-enginehandler--入口)
   - 4.2 [AppMsg](#42-appmsg--消息枚举)
   - 4.3 [EventDriver & FrameEvents](#43-eventdriver--frameevents)
   - 4.4 [KeyState](#44-keystate--按键状态位掩码)
   - 4.5 [KeyboardInput](#45-keyboardinput)
   - 4.6 [MouseInput & MouseButton](#46-mouseinput--mousebutton)
   - 4.7 [Timer](#47-timer)
   - 4.8 [D3D11](#48-d3d11)
   - 4.9 [StateObjects](#49-stateobjects)
   - 4.10 [TextureInfo & d3d11_utils](#410-textureinfo--d3d11_utils)
   - 4.11 [Sprite2D / Sprite2DObject / Sprite2DBuffer / HaveID](#411-sprite2d--sprite2dobject--sprite2dbuffer--haveid)
   - 4.12 [TextureInfoArced](#412-textureinfoorced)
   - 4.13 [SpriteBatch2D & Pipeline](#413-spritebatch2d--pipeline-trait)
   - 4.14 [ShapeBatch2D](#414-shapebatch2d)
   - 4.15 [Transform2D](#415-transform2d)
   - 4.16 [Camera2D](#416-camera2d)
   - 4.17 [Collider / ColliderInstance / Overlap](#417-collider--colliderinstance--overlap)
   - 4.18 [AtlasText & TextLayout](#418-atlastext--textlayout)
5. [完整示例](#5-完整示例)
6. [常见陷阱与易错点](#6-常见陷阱与易错点)
7. [详细使用教程](#7-详细使用教程)
   - 7.1 [app_shapes —— 纯形状渲染入门](#71-app_shapes--纯形状渲染入门)
   - 7.2 [app_sethsweeper —— 纹理精灵+文字+碰撞综合示例](#72-app_sethsweeper--纹理精灵文字碰撞综合示例)
   - 7.3 [app_fish —— 完整的双人游戏](#73-app_fish--完整的双人游戏)
8. [附录：已知问题与未来规划](#8-附录已知问题与未来规划)

---

## 1. 概述与架构

### 1.1 线程模型

引擎使用**双线程架构**：

| 线程 | 职责 | 组件 |
|------|------|------|
| **Main Thread** | winit 事件循环、窗口管理 | `EngineHandler` |
| **App Thread** | 输入处理、物理、渲染 | `EventDriver` + 你的 `App` |

- `EngineHandler` 在 `resumed()` 时创建窗口、建立 MPSC 通道、派生 App 线程
- 窗口事件（键盘、鼠标、窗口大小）通过通道转发到 App 线程
- App 线程通过 `EventDriver::poll_frame()` 一次性取出所有待处理事件

```
Main Thread (winit)          App Thread
┌────────────────┐          ┌──────────────────────┐
│  EngineHandler │──MPSC──→│  EventDriver         │
│  (Application- │  msg     │  ├─ KeyboardInput    │
│   Handler)     │          │  ├─ MouseInput       │
│                │          │  └─ poll_frame()     │
└────────────────┘          └──────────┬───────────┘
                                       │
                              ┌────────▼───────────┐
                              │  App (your code)    │
                              │  ├─ update_tiles()  │
                              │  ├─ render_frame()  │
                              │  └─ …              │
                              └────────┬────────────┘
                                       │
                              ┌────────▼───────────┐
                              │  D3D11 / Sprite-    │
                              │  Batch2D / Shape-   │
                              │  Batch2D            │
                              └─────────────────────┘
```

### 1.2 帧循环

```
┌──────────┐    ┌───────────┐    ┌─────────┐    ┌──────────┐
│ poll_    │───→│ update    │───→│ render  │───→│ present  │
│ frame()  │    │ (物理/    │    │ (精灵/   │    │ (交换链  │
│          │    │  输入)    │    │  形状)   │    │  提交)   │
└──────────┘    └───────────┘    └─────────┘    └──────────┘
                                                    │
                                              ┌─────▼─────┐
                                              │ end_frame │
                                              │ (清除边缘  │
                                              │  状态)     │
                                              └───────────┘
```

### 1.3 平台支持

当前仅支持 **Windows (x64)**，基于 Direct3D 11。

⚠️ **Warning**：引擎使用了大量 Windows/Win32 API，跨平台移植需要重写整个 `graphic::d3d11` 模块。

---

## 2. 核心概念

本章解释引擎的几个关键设计理念，理解这些概念有助于避免常见的错误用法。

### 2.1 双线程架构解析

引擎使用双线程架构的根本原因在于 **winit 事件循环必须运行在主线程**，而 D3D11 渲染和游戏逻辑可以放在另一个线程独立运行。

**为什么 winit 要求主线程？**
- Windows 窗口消息泵（message pump）必须在创建窗口的线程上运行
- 所有窗口事件（WM_PAINT、WM_SIZE、WM_INPUT 等）都发送到创建窗口的线程消息队列

**为什么 App 线程独立？**
- 渲染循环可能阻塞（如等待垂直同步），如果放在主线程会卡死窗口事件处理
- 游戏物理更新需要稳定的帧率，不受窗口事件处理的影响

**通道（MPSC）的权衡**：
- `poll_frame()` 使用 `try_recv()` 而非 `recv()`，原因是不能阻塞等待事件——即使没有事件也要继续渲染
- 如果通道积压事件（例如用户快速按键），`poll_frame()` 会一次性处理所有待处理事件
- 这意味着输入响应可能延迟最多一帧

### 2.2 批渲染原理

`SpriteBatch2D` 和 `ShapeBatch2D` 都是批渲染器（Batch Renderer），它们的工作原理是：

**什么是批渲染？**
- 将多个精灵/形状的顶点数据收集到一个大缓冲区中
- 一次 `Draw` 调用绘制所有内容
- 减少 CPU↔GPU 通信的上下文切换开销

**内部结构**：
- 每个精灵由 **4 个顶点** 组成一个四边形（quad）
- 顶点索引使用 **16-bit 无符号整数**（`u16`）
- 因此每个批的最大顶点数为 `0xFFFF = 65535`，即最多 `65535 / 4 = 16383` 个精灵

**为什么 capacity 有限制？**
```
capacity ≤ 0xFFFF / 4 = 16383
```
如果你的精灵数超过 capacity，`push_buffered` 会自动拆分为多个 draw call。虽然仍然正确，但会降低性能。

**ShapeBatch2D 的不同**：
- `ShapeBatch2D` 不是 4 顶点 quad，而是任意三角形网格
- 它使用 `remap` 算法对顶点去重（避免重复顶点占用缓冲区空间）
- 每次 `submit_and_draw` 都会重建顶点缓冲区，因此大量三角形时性能下降

### 2.3 Pipeline 模式与排序

`Sprite2DBuffer` 的排序逻辑是引擎设计的核心：

```
排序规则：先按 layer 升序，再按 pipeline.get_id() 升序
```

**为什么需要 pipeline 分组？**
- 在每个 draw call 之前，D3D11 需要设置渲染状态（纹理、着色器等）
- 状态切换是昂贵的操作
- 通过按 pipeline（即纹理）排序，相同的纹理只需要设置一次状态，然后绘制所有使用该纹理的精灵
- 这称为 **状态排序（State Sorting）**

**layer 的作用**：
- `layer` 控制绘制顺序，值越小越先绘制
- 通常背景使用低 layer（如 0.0），前景使用高 layer（如 100.0）
- 文字阴影的 layer 低于文字本身（如 99.0 vs 100.0），确保阴影在文字下方

**`push_buffered` 的工作流程**：
1. 内部对 `Sprite2DBuffer` 进行排序（如果尚未排序）
2. 遍历所有精灵，当 `pipeline.get_id()` 变化时：
   - 调用 `submit_and_draw` 提交当前批
   - 调用 `clear_batch` 清空
   - 对新 pipeline 调用 `apply_to_batch`（即 `set_texture`）
3. 将精灵添加到当前批
4. 遍历结束后，提交最后一批

### 2.4 边缘检测的位运算原理

`KeyState` 使用一个 `u8` 位掩码来表示按键状态：

```
位 0 (0b0001): 按下标志
位 1 (0b0010): 边缘标志
位 2 (0b0100): 真边缘标志
位 3 (0b1000): 突然释放标志
```

**普通边缘 vs 真边缘**：

| 状态 | 二进制 | 含义 |
|------|--------|------|
| 释放中 | `0000` | 按键已释放 |
| 按下中 | `0001` | 按键当前按下 |
| 上升沿 | `0010` | 刚被释放（边缘事件） |
| 下降沿 | `0011` | 刚被按下（边缘事件） |
| 真上升沿 | `0110` | 确实刚被释放（去抖） |
| 真下降沿 | `0111` | 确实刚被按下（去抖） |

**为什么需要真边缘？**
- 某些操作系统在按住按键时会连续发送 Pressed 事件（键盘重复）
- 普通边缘每次事件都会触发，导致 `is_down_edge()` 在同一帧返回 `true` 多次
- 真边缘只在状态**确实从 Released 变为 Pressed**（或反之）时触发一次

**`off_edge()` 的工作原理**：
- 每帧结束后，`end_frame()` 对所有按键调用 `off_edge()`
- `off_edge()` 清除位 1 和位 2（`!(0b0010 | 0b0100)`）
- 这样下一帧开始时，所有边缘标志都被重置
- 这就是为什么必须在 `end_frame()` **之前**完成所有输入处理

### 2.5 MPSC 通道时序

引擎使用 Rust 标准库的 `mpsc::channel`（Multi-Producer, Single-Consumer）：

```
主线程 (发送端)              App 线程 (接收端)
    │                           │
    │── send(AppMsg) ────────→  │ poll_frame()
    │                           │   ├─ try_recv() → Ok(msg) → 处理
    │── send(AppMsg) ────────→  │   ├─ try_recv() → Ok(msg) → 处理
    │                           │   ├─ try_recv() → Empty → 返回
    │                           │
    │                    update() → render() → present()
    │                           │
    │                           │ end_frame()
    │                           │
```

**关键特性**：
- `try_recv()` 是非阻塞的：如果通道为空，立即返回 `Empty`
- `poll_frame()` 在一个循环中反复调用 `try_recv()`，直到通道为空
- 这意味着 `poll_frame()` **不会等待**事件——它处理完当前所有积压事件后立即返回
- 因此引擎的帧率由 `present()` 的垂直同步决定，而不是由输入事件驱动

**与事件驱动模型的区别**：
- 传统事件驱动：等待事件 → 处理事件 → 渲染
- 本引擎：轮询事件 → 处理所有积压事件 → 更新 → 渲染

---

## 3. 快速开始

### 3.1 创建 Workspace 和项目

```toml
[workspace]
members = ["krjw_engine", "my_app"]   # krjw_engine 是引擎本体，my_app 是你的应用
resolver = "3"
```

### 3.2 在 `my_app/Cargo.toml` 中添加依赖

```toml
[package]
name = "my_app"
version = "0.1.0"
edition = "2024"

[dependencies]
krjw_engine = { path = "../krjw_engine" }  # 引用引擎
anyhow = "1.0"                              # 错误处理
glam = "0.29"                               # 数学库（Vec2、Mat4 等）
```

💡 **说明**：如果不需要精灵纹理，可以省略 `image` 和 `cosmic-text`；如果不需要音频，可以省略 `kira`。但引擎本身的 `Cargo.toml` 已包含所有依赖，你的应用只需引用 `krjw_engine` 即可间接使用这些库。

### 3.3 最小入口 `my_app/src/main.rs` —— 逐行详解

```rust
// 声明 app 模块（对应 app.rs 文件）
mod app;

// 引入 EngineHandler —— 主线程的 winit 事件处理器
use krjw_engine::EngineHandler;
// ControlFlow::Poll 表示每帧轮询（非阻塞等待事件）
use krjw_engine::winit::event_loop::{ControlFlow, EventLoop};

fn main() {
    // ── 1. 创建 winit 事件循环 ──
    // EventLoop 是 winit 的核心，负责从操作系统接收窗口事件
    let event_loop = EventLoop::new().unwrap();

    // 设置事件循环的控制流为 Poll（轮询模式）
    // 另一种选择是 ControlFlow::Wait，会在没有事件时休眠
    event_loop.set_control_flow(ControlFlow::Poll);

    // ── 2. 创建 EngineHandler ──
    // EngineHandler::new 接受一个闭包，该闭包在窗口创建后**在派生线程中**执行
    // 闭包参数：
    //   - window: winit::window::Window —— 创建的窗口
    //   - hwnd: isize —— Windows 窗口句柄（用于 D3D11 初始化）
    //   - rx: Receiver<AppMsg> —— 从主线程接收事件的 MPSC 接收端
    let mut handler = EngineHandler::new(|window, hwnd, rx| {
        // 在这里初始化你的应用并运行主循环
        let mut app = app::App::default();
        app.run(window, hwnd, rx)
    });

    // ── 3. 运行事件循环 ──
    // run_app 会阻塞当前线程，直到窗口关闭
    // 内部会调用 handler 的 resumed() 创建窗口和 App 线程
    event_loop.run_app(&mut handler).unwrap();
}
```

**执行流程总结**：
1. `main()` 创建 `EventLoop` 和 `EngineHandler`
2. `run_app()` 开始事件循环
3. 事件循环触发 `EngineHandler::resumed()`：
   - 创建窗口（标题 "KrisuRJW"，960×600）
   - 解析 HWND
   - 创建 MPSC 通道
   - 派生 App 线程，在新线程中执行闭包
4. App 线程中运行 `app.run(window, hwnd, rx)` —— 你的主循环
5. 主线程继续处理窗口事件并通过通道发送 `AppMsg`

### 3.4 应用骨架 `my_app/src/app.rs` —— 逐行详解

```rust
use std::sync::mpsc::Receiver;
use anyhow::Result;
use krjw_engine::*;  // 引擎的公开 API

// ── App 结构体 ──
// 使用 Option<AppContext> 模式是因为 App 需要在 run() 中才能创建引擎资源
pub struct App {
    pub ctx: Option<AppContext>,
}

// ── AppContext ──
// 存放所有需要在帧循环中访问的引擎资源和游戏状态
// 放在单独的 struct 中可以方便地在 run() 中统一初始化
pub struct AppContext {
    // 引擎核心
    pub window: winit::window::Window,     // winit 窗口
    pub gfx: D3D11,                         // Direct3D 11 设备封装
    pub batch: SpriteBatch2D,               // 精灵批渲染器（纹理）
    pub shape_batch: ShapeBatch2D,          // 形状批渲染器（无纹理）
    pub textures: std::collections::HashMap<String, std::sync::Arc<TextureInfo>>, // 纹理管理器
    pub camera: Camera2D,                   // 正交相机
    pub timer: Timer,                       // 帧计时器
    pub text_buf: Sprite2DBuffer<TextureInfoArced, Transform2D>,   // 文字精灵缓冲区
    pub sprite_buf: Sprite2DBuffer<TextureInfoArced, Transform2D>,  // 普通精灵缓冲区
    // 游戏状态……
}

impl Default for App {
    fn default() -> Self {
        // 初始时 AppContext 尚未创建，设为 None
        App { ctx: None }
    }
}

impl App {
    // ── run() 方法 ──
    // 在 App 线程中执行，完成所有初始化并进入主循环
    pub fn run(&mut self, window: winit::window::Window, hwnd: isize,
               rx: Receiver<AppMsg>) -> Result<()> {
        // ═══ 初始化阶段 ═══

        // 1. D3D11 初始化
        //    hwnd 是 Windows 窗口句柄，D3D11 需要它来创建交换链
        let gfx = D3D11::init_on_hwnd(hwnd)?;
        let size = window.inner_size();

        // 2. 事件驱动
        //    EventDriver 负责从 MPSC 通道读取 AppMsg，更新输入状态
        let mut driver = EventDriver::new(rx);
        driver.set_initial_window_size(size.width, size.height);

        // 3. 创建批渲染器
        //    SpriteBatch2D: 用于渲染带纹理的精灵（使用 ps_tex_rgba_2d 着色器）
        //    ShapeBatch2D: 用于渲染纯色形状（使用 ps_solid_2d 着色器）
        //    两者的顶点着色器使用同一个 vs_puc_m_2d
        let batch = SpriteBatch2D::new(&gfx.device, 2048,
            &gfx.states.vs_puc_m_2d, &gfx.states.ps_tex_rgba_2d,
            &gfx.states.input_layout_puc)?;
        let shape_batch = ShapeBatch2D::new(&gfx.device, 4096,
            &gfx.states.vs_puc_m_2d, &gfx.states.ps_solid_2d,
            &gfx.states.input_layout_puc)?;

        // ═══ 主循环 ═══
        loop {
            // 4. 轮询帧事件
            let events = driver.poll_frame();
            // close_requested: 用户点击关闭按钮
            // disconnected: 主线程已关闭通道（EngineHandler 被 Drop）
            if events.close_requested || events.disconnected { break; }

            // 5. 窗口大小变化处理
            //    if_window_size_dirty 会在窗口大小变化时自动执行闭包
            //    如果不需要 window_size_dirty 标志，可以用 if_window_size_dirty
            driver.if_window_size_dirty(|w, h| {
                gfx.on_resize(w, h)?;   // 重建交换链的 back buffer
                Ok(())
            })?;

            // 6. 你的帧逻辑
            //    update() 处理输入、物理、游戏逻辑
            //    render() 清屏、绘制精灵和形状、present
            // self.update()?;
            // self.render(&gfx, &camera)?;

            // 7. 提交渲染结果
            gfx.present()?;

            // 8. 结束帧：清除边缘触发状态
            //    必须在 present() 之后调用，否则下一帧的边缘检测会出错
            driver.end_frame();
        }
        Ok(())
    }
}
```

**关键设计原则**：
- `AppContext` 模式：将引擎资源和游戏状态集中管理，避免在 `App` 结构体中出现大量字段
- `Driver` 的生命周期：`driver` 在 `run()` 中创建，主循环中持有可变引用，通过 `driver.end_frame()` 推进边缘状态
- `Result<()>` 传播：所有可能失败的操作都返回 `Result`，使用 `?` 操作符向上传播错误

### 3.5 添加窗口属性定制（进阶）

如果你需要自定义窗口标题、尺寸或透明模式，可以在 `EngineHandler::new` 中传入 `WindowAttributes`：

```rust
use krjw_engine::winit::dpi::LogicalSize;
use krjw_engine::winit::window::WindowAttributes;

let mut handler = EngineHandler::new(
    WindowAttributes::default()
        .with_title("我的游戏")                    // 自定义标题
        .with_inner_size(winit::dpi::Size::Logical(LogicalSize {
            width: 960.0,
            height: 600.0,
        }))
        .with_transparent(true),                   // 允许透明窗口
    |window, hwnd, rx| {
        // ...
    }
);
```

> **注意**：`EngineHandler::new` 的第一个参数是 `WindowAttributes`，第二个是闭包。在早期版本的文档中（`DOC_TUTORIAL.md` 的部分示例），`EngineHandler::new` 只接受一个闭包参数；最新版本已改为接受两个参数。

---

## 4. 模块详解

---

### 4.1 EngineHandler — 入口

**文件**：`engine_handler.rs`  
**公开导出**：`krjw_engine::EngineHandler`

主线程的 winit 事件处理器。负责创建窗口、建立 MPSC 通道、派生应用线程。

#### 构造

```rust
pub fn new(
    init_window_attrib: WindowAttributes,
    app_init: impl FnOnce(Window, isize, Receiver<AppMsg>) -> Result<()>
        + Send + 'static
) -> Self;
```

- `init_window_attrib`：win32 窗口属性（标题、尺寸等）
- `app_init`：在派生线程中调用的闭包，接收 `(Window, HWND isize, Receiver<AppMsg>)`
- 闭包应运行你的 App 主循环，返回 `Result<()>`

#### 内部行为

- `resumed()` 时根据 `init_window_attrib` 创建窗口，解析 HWND，创建通道，派生线程
- `window_event()` 将所有 WindowEvent 转为 `AppMsg` 并发送到通道
- `device_event()` 将 `MouseMotion` 事件转发
- `Drop` 时发送通道关闭信号

#### Example

```rust
fn main() {
    let event_loop = winit::event_loop::EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut handler = EngineHandler::new(
        WindowAttributes::default()
            .with_title("我的应用")
            .with_inner_size(winit::dpi::Size::Logical(LogicalSize { width: 960.0, height: 600.0 })),
        |window, hwnd, rx| {
            let mut app = app::App::default();
            app.run(window, hwnd, rx)
        }
    );
    event_loop.run_app(&mut handler).unwrap();
}
```

⚠️ **Warning**：
- 关闭窗口时调用 `event_loop.exit()`，可能导致 App 线程仍在运行就被终止
- 仅支持 Win32 平台，非 Windows 系统会 panic

🚧 **TODO**：
- 提供更优雅的退出机制（等待 App 线程自然结束）
- 跨平台窗口句柄支持

---

### 4.2 AppMsg — 消息枚举

**文件**：`msg.rs`  
**公开导出**：`krjw_engine::AppMsg`

主线程通过 MPSC 通道发送给 App 线程的所有事件类型。

```rust
#[derive(Debug, Clone, Copy)]
pub enum AppMsg {
    CloseRequested,                      // 窗口关闭
    Resized(u32, u32),                   // 窗口大小变化 (width, height)
    Moved(i32, i32),                     // 窗口移动 (x, y)
    KeyboardInput { key_code: KeyCode, state: ElementState },
    CursorMoved(f64, f64),               // 光标移动 (x, y)
    CursorEntered,                       // 光标进入窗口
    CursorLeft,                          // 光标离开窗口
    MouseWheel(f64, f64),                // 滚轮行增量 (x, y)
    MouseWheelPixel(f64, f64),           // 滚轮像素增量 (x, y)
    MouseInput { button: MouseButton, state: ElementState },
    MouseMotion(f64, f64),               // 鼠标原生移动增量 (dx, dy)
}
```

所有变体均使用 `Send + Copy` 类型，因此 `AppMsg` 自身也是 `Send`。

---

### 4.3 EventDriver & FrameEvents

**文件**：`event_driver.rs`  
**公开导出**：`krjw_engine::EventDriver`, `krjw_engine::FrameEvents`

App 线程的消息驱动，从通道接收消息并更新输入/窗口状态。

#### FrameEvents

```rust
pub struct FrameEvents {
    pub close_requested: bool,   // 是否收到关闭请求
    pub disconnected: bool,      // 通道是否断开（发送端丢弃）
}
```

#### 方法

```rust
impl EventDriver {
    pub fn new(rx: Receiver<AppMsg>) -> Self;
    pub fn set_initial_window_size(&mut self, w: u32, h: u32);

    // 帧管理
    pub fn poll_frame(&mut self) -> FrameEvents;   // 取出所有待处理消息
    pub fn end_frame(&mut self);                   // 推进边缘状态

    // 输入访问
    pub fn keyboard(&self) -> &KeyboardInput;
    pub fn mouse(&self) -> &MouseInput;

    // 窗口状态
    pub fn window_pos(&self) -> (i32, i32);
    pub fn window_size(&self) -> (u32, u32);
    pub fn if_window_size_dirty<F>(&mut self, then: F) -> Result<()>;
    //                                    ^^^ 闭包方式处理 resize
    //  替代方案：检查 + 清除
    //  pub fn window_size_dirty(&self) -> bool;
    //  pub fn clear_window_size_dirty(&mut self);
}
```

> **注意**：`if_window_size_dirty` 是推荐的使用方式，它同时检查 `window_size_dirty` 标志、执行闭包、并清除标志。如果你的 resize 处理逻辑不复杂，使用这个方法可以减少代码量。

#### Example

```rust
loop {
    let events = driver.poll_frame();
    if events.close_requested || events.disconnected { break; }

    // 推荐方式：使用 if_window_size_dirty
    driver.if_window_size_dirty(|w, h| {
        gfx.on_resize(w, h)?;
        camera.viewport_size = Vec2::new(w as f32, h as f32);
        Ok(())
    })?;

    // 使用输入
    let ks = driver.keyboard().get_key_state(KeyCode::KeyW);
    let mouse_pos = driver.mouse().get_mouse_pos_vec2();

    driver.end_frame();
}
```

⚠️ **Warning**：`poll_frame()` 使用 `try_recv()` 非阻塞读取，不会等待消息。如果你的 App 逻辑与帧率不同步（例如需要等待输入才更新），需要自行实现同步机制。

---

### 4.4 KeyState — 按键状态位掩码

**文件**：`key_state.rs`  
**公开导出**：`krjw_engine::KeyState`, `krjw_engine::KEY_STATE_*` 常量

使用位掩码表示按键的当前状态，支持按下/释放检测和边缘触发。

#### 位掩码常量

| 常量 | 值 | 含义 |
|------|-----|------|
| `KEY_STATE_RELEASED` | 0b0000 | 已释放 |
| `KEY_STATE_PRESSING` | 0b0001 | 按下中 |
| `KEY_STATE_UP_EDGE` | 0b0010 | 刚释放（普通边缘） |
| `KEY_STATE_DOWN_EDGE` | 0b0011 | 刚按下（普通边缘） |
| `KEY_STATE_UP_TRUE_EDGE` | 0b0110 | 刚释放（真边缘） |
| `KEY_STATE_DOWN_TRUE_EDGE` | 0b0111 | 刚按下（真边缘） |

#### 方法

```rust
impl KeyState {
    pub fn is_pressed(&self) -> bool;
    pub fn is_released(&self) -> bool;
    pub fn is_edge(&self) -> bool;          // 是否有边缘触发
    pub fn is_true_edge(&self) -> bool;     // 是否是真边缘
    pub fn is_up_edge(&self) -> bool;       // 是否上升沿
    pub fn is_down_edge(&self) -> bool;     // 是否下降沿
    pub fn is_up_true_edge(&self) -> bool;
    pub fn is_down_true_edge(&self) -> bool;
    pub fn off_edge(&self) -> KeyState;     // 清除边缘位（每帧结束后调用）
}
```

#### 边缘检测（Edge Detection）说明

- **普通边缘**（`DOWN_EDGE` / `UP_EDGE`）：OS 报告的状态变化。某些 OS 可能连续发送相同状态的重复事件。
- **真边缘**（`DOWN_TRUE_EDGE` / `UP_TRUE_EDGE`）：仅当状态确实从相反状态切换时才为真。用于需要"恰好按一次"的场景（如跳跃、射击）。

| 场景 | 普通边缘 | 真边缘 |
|------|---------|-------|
| 按住按键 → 连续收到 Pressed 事件 | 每帧触发 | 仅第一次触发 |
| 实际按下一次 | 触发一次 | 触发一次 |
| 去抖需求 | ❌ | ✅ |

#### Example

```rust
let ks = driver.keyboard().get_key_state(KeyCode::KeyW);

// 持续按住（模拟移动）
if ks.is_pressed() {
    player.move_forward(dt);
}

// 单次触发（跳跃、射击）
if ks.is_down_true_edge() {
    player.jump();
}
```

⚠️ **Warning**：`is_down_edge()` 和 `is_down_true_edge()` 的返回在每帧结束后通过 `off_edge()` 清除。如果在同一帧内多次查询同一个 KeyState，返回结果一致。确保在 `end_frame()` 之前完成所有输入处理。

---

### 4.5 KeyboardInput

**文件**：`keyboard_input.rs`  
**公开导出**：`krjw_engine::KeyboardInput`

管理所有键盘按键的状态，使用 `HashMap<KeyCode, KeyState>`。

#### 方法

```rust
impl KeyboardInput {
    pub fn get_key_state(&self, key_code: KeyCode) -> KeyState;
    pub fn get_keys_iter(&self) -> impl Iterator<Item = (KeyCode, KeyState)> + '_;
    pub fn end_frame(&mut self);        // 清除所有边缘位
}
```

#### 建议辅助宏（在你的 `app.rs` 中定义）

```rust
macro_rules! key_pressed { ($driver:expr, $key:expr) => {
    $driver.keyboard().get_key_state($key).is_pressed()
}}
macro_rules! key_state { ($driver:expr, $key:expr) => {
    $driver.keyboard().get_key_state($key)
}}
```

#### 使用 Example

```rust
// 同时检测多个按键
if key_pressed!(driver, KeyCode::KeyW) {
    pos.y -= speed * dt;
}
if key_pressed!(driver, KeyCode::KeyS) {
    pos.y += speed * dt;
}
if key_pressed!(driver, KeyCode::KeyA) {
    pos.x -= speed * dt;
}
if key_pressed!(driver, KeyCode::KeyD) {
    pos.x += speed * dt;
}

// 真边缘检测（单次事件）
if key_state!(driver, KeyCode::Space).is_down_true_edge() {
    fire_bullet();
}
```

---

### 4.6 MouseInput & MouseButton

**文件**：`mouse_input.rs`  
**公开导出**：`krjw_engine::MouseInput`, `krjw_engine::MouseButton`

管理鼠标位置、按钮状态、滚轮增量。

#### MouseButton

```rust
pub enum MouseButton {
    Left = 0,
    Right = 1,
    Middle = 2,
    X1 = 3,    // 后退键
    X2 = 4,    // 前进键
}
```

#### MouseInput 方法

```rust
impl MouseInput {
    pub fn get_mouse_position(&self) -> (f64, f64);
    pub fn get_mouse_pos_vec2(&self) -> Vec2;          // 便捷返回 glam::Vec2
    pub fn get_mouse_delta(&self) -> (f64, f64);        // 原生鼠标移动增量
    pub fn get_mouse_button_state(&self, button: MouseButton) -> KeyState;
    pub fn get_mouse_wheel_delta(&self) -> (f64, f64);  // 滚轮行在一帧内的增量
    pub fn get_pixel_wheel(&self) -> Option<(f64, f64)>; // 滚轮像素在一帧内增量（触控板等），如果不可用则返回 None，否则返回 Some((f64, f64))，分别代表 x、y 方向的像素增量
    pub fn is_in_window(&self) -> bool;
    pub fn get_mouse_button_states_iter(&self) -> impl Iterator<Item = (MouseButton, KeyState)> + '_;
    pub fn end_frame(&mut self);   // 清除边缘位和滚轮增量
}
```

#### Example

```rust
// 获取鼠标位置
let mouse_pos = driver.mouse().get_mouse_pos_vec2();

// 鼠标左键点击检测
if driver.mouse().get_mouse_button_state(MouseButton::Left).is_down_true_edge() {
    println!("Left click at {:?}", mouse_pos);
}

// 滚轮缩放
let (_, wheel_y) = driver.mouse().get_mouse_wheel_delta();
camera.zoom *= 1.0 - (wheel_y as f32) * 0.1;

// 鼠标中键拖拽
if driver.mouse().get_mouse_button_state(MouseButton::Middle).is_pressed() {
    let (dx, dy) = driver.mouse().get_mouse_delta();
    camera.position.x -= dx as f32;
    camera.position.y -= dy as f32;
}
```

⚠️ **Warning**：
- `mouse_delta` 来自 `DeviceEvent::MouseMotion`，是**原始增量**，不受操作系统鼠标加速/指针速度影响。适用于 FPS 相机控制。
- `mouse_position` 来自 `WindowEvent::CursorMoved`，是窗口坐标。
- `pixel_wheel` 和 `mouse_wheel_delta` 每帧结束时被清零，请在 `end_frame()` 前使用。

---

### 4.7 Timer

**文件**：`timer.rs`  
**公开导出**：`krjw_engine::Timer`

帧计时器，提供 Delta Time 和 FPS 统计。

#### 方法

```rust
impl Timer {
    pub fn pre_frame_and_get_delta_time(&mut self) -> f64; // 返回距上一帧的秒数
    pub fn post_frame_fpsc(&mut self, dt: f64);            // 更新 FPS（EMA 平滑）
    pub fn get_fps(&self) -> f64;                          // 获取当前平滑 FPS
}
```

#### Example

```rust
// 帧开始
let dt = timer.pre_frame_and_get_delta_time();
// 如果你希望 dt 有上限（防止大跳帧时间步长过大）：
let dt = dt.min(0.05);

// 更新逻辑
player.update(dt);

// 帧结束
timer.post_frame_fpsc(dt);

// 显示 FPS
println!("FPS: {:.1}", timer.get_fps());
```

#### FPS 计算公式

使用指数移动平均（EMA），α = 0.1：
```
instant_fps = 1.0 / dt
fps = fps * 0.9 + instant_fps * 0.1
```

---

### 4.8 D3D11

**文件**：`graphic/d3d11.rs`  
**公开导出**：`krjw_engine::D3D11`

Direct3D 11 设备封装，管理设备、交换链、渲染目标、深度模板。

#### 公开字段

```rust
pub struct D3D11 {
    pub device: ID3D11Device,          // D3D 设备
    pub swap_chain: IDXGISwapChain,    // 交换链
    pub imm_context: ID3D11DeviceContext, // 即时上下文
    pub states: StateObjects,          // 预创建的渲染状态对象
    // (以下为私有字段)
    // render_target_view
    // depth_stencil_texture
    // depth_stencil_view
}
```

#### 方法

```rust
impl D3D11 {
    // 初始化
    pub fn init_on_hwnd(hwnd: isize) -> Result<Self>; // isize 对应 winit 库对 HWND 的定义
    pub fn init_on_window(window: &Window) -> Result<Self>;

    // 渲染控制
    pub fn clear_screen(&self, color_rgba: &[f32; 4]);
    pub fn set_viewport(&self, top_x: f32, top_y: f32, width: f32, height: f32);
    pub fn present(&self) -> Result<()>;

    // 窗口大小变化
    pub fn on_resize(&mut self, width: u32, height: u32) -> Result<()>;
    pub fn reset_rtv(&mut self, width: u32, height: u32, format: DXGI_FORMAT, flags: DXGI_SWAP_CHAIN_FLAG) -> Result<()>;
    pub fn reset_dsv(&mut self, width: u32, height: u32) -> Result<()>;

    // 视图访问
    pub fn rtv(&self) -> &ID3D11RenderTargetView;
    pub fn dsv(&self) -> &ID3D11DepthStencilView;
}
```

#### Example

```rust
// 初始化
let gfx = D3D11::init_on_hwnd(HWND(hwnd as *mut _))?;

// 帧渲染
gfx.clear_screen(&[0.1, 0.1, 0.2, 1.0]);

// ... 绘制内容 ...

gfx.present()?;

// 窗口大小变化
gfx.on_resize(new_width, new_height)?;
```

⚠️ **Warning**：
- `init_on_hwnd` 仅支持 Win32 窗口，没有跨平台能力
- `present()` 固定使用 `1` 作为 `SyncInterval`（等待垂直同步），不可配置
- `init_on_hwnd` 中硬编码创建了 2 个后备缓冲（`BufferCount: 2`）+ `DXGI_SWAP_EFFECT_FLIP_DISCARD`
- Debug 模式下自动启用 `D3D11_CREATE_DEVICE_DEBUG`，如果没有安装 Direct3D Debug Layer 会报错

#### ❌ 常见错误

| 错误 | 后果 | 正确做法 |
|------|------|---------|
| `init_on_hwnd(hwnd)` 忘记转换类型 | 编译错误 | `D3D11::init_on_hwnd(HWND(hwnd as *mut _))` |
| 窗口 resize 后忘记调用 `on_resize` | 渲染画面拉伸或黑屏 | 在 `if_window_size_dirty` 中调用 `gfx.on_resize(w, h)` |
| 假设 `present()` 不阻塞 | 帧率不可控 | `SyncInterval=1` 固定等待垂直同步 |

---

### 4.9 StateObjects

**文件**：`graphic/d3d11/state_objects.rs`  
在 `D3D11::states` 中访问，自动创建。

#### 所有预创建资源

| 类别 | 名称 | 说明 |
|------|------|------|
| **混合状态** | `blend_opaque` | 不透明（Blend 关闭） |
| | `blend_alpha` | Alpha 透明（`SRC_ALPHA, INV_SRC_ALPHA`） |
| | `blend_additive` | 叠加混合（`SRC_ALPHA, ONE`） |
| **采样器** | `sampler_point_clamp` | 点采样 + Clamp |
| | `sampler_linear_clamp` | 线性采样 + Clamp |
| | `sampler_linear_wrap` | 线性采样 + Wrap |
| **光栅化** | `rasterizer_solid_cull_none` | 实体无剔除 |
| | `rasterizer_solid_cull_back` | 实体背面剔除 |
| | `rasterizer_wireframe` | 线框模式 |
| **深度模板** | `depth_none` | 深度测试关闭 |
| | `depth_less` | 深度测试开启（Less） |
| **纹理** | `white_texture_srv` | 1×1 白色像素纹理（用于纯色渲染） |
| **着色器** | `vs_puc_m_2d` | 2D 顶点着色器（pos + uv + color + MVP） |
| | `ps_solid_2d` | 纯色像素着色器（忽略纹理） |
| | `ps_tex_rgba_2d` | RGBA 纹理像素着色器 |
| | `ps_tex_r8_2d` | R8 单通道纹理像素着色器 |
| **输入布局** | `input_layout_puc` | POSITION + TEXCOORD + COLOR 布局 |

#### Example — 设置混合状态

```rust
// 使用 alpha 混合
unsafe {
    gfx.imm_context.OMSetBlendState(
        Some(&gfx.states.blend_alpha),
        None,
        0xFFFFFFFF,
    );
}

// 使用线框模式调试
unsafe {
    gfx.imm_context.RSSetState(Some(&gfx.states.rasterizer_wireframe));
}
```

⚠️ **Warning**：引擎 `vs_puc_m_2d` 和 `ps_*` 着色器通过 `include_bytes!` 硬编码，没有提供创建自定义着色器的接口。如果需要自定义着色器，需要手动调用 `d3d11_utils` 的 `create_vs` / `create_ps` 函数。

---

### 4.10 TextureInfo & d3d11_utils

**文件**：`graphic/d3d11/d3d11_utils.rs`  
**公开导出**：`krjw_engine::TextureInfo`

工具函数模块，提供纹理、缓冲区、着色器的创建。

#### TextureInfo

```rust
pub struct TextureInfo {
    pub texture: ID3D11Texture2D,
    pub srv: ID3D11ShaderResourceView,
    pub width: u32,
    pub height: u32,
    pub format: DXGI_FORMAT,
}

impl TextureInfo {
    pub fn size_vec2f(&self) -> Vec2;   // 返回 (width, height) 作为 Vec2
}
```

#### 关键工具函数

```rust
// 着色器
pub fn compile_shader(source: &[u8], entrypoint: PCSTR, target: PCSTR) -> Result<Vec<u8>>;
pub fn create_vs(device: &ID3D11Device, hlsl_bytes: &[u8]) -> Result<ID3D11VertexShader>;
pub fn create_ps(device: &ID3D11Device, hlsl_bytes: &[u8]) -> Result<ID3D11PixelShader>;
pub fn create_input_layout(device, desc: &[D3D11_INPUT_ELEMENT_DESC], vs_blob: &[u8]) -> Result<ID3D11InputLayout>;

// 缓冲区
pub fn create_dynamic_buffer(device, byte_width, bind_flags) -> Result<ID3D11Buffer>;
pub fn create_immutable_buffer(device, data, bind_flags) -> Result<ID3D11Buffer>;
pub fn create_constant_buffer<T>(device) -> Result<ID3D11Buffer>;
pub fn write_buffer<T>(context, buffer, data) -> Result<()>;
pub fn as_u8_slice<T>(data: &[T]) -> &[u8];

// 纹理
pub fn create_texture_2d(device, width, height, format, bind_flags, usage, cpu_access, initial_data) -> Result<ID3D11Texture2D>;
pub fn create_srv(device, texture, format) -> Result<ID3D11ShaderResourceView>;
pub fn load_texture_from_dynamic_image(device, img: &DynamicImage) -> Result<TextureInfo>;
```

#### Example — 从图片文件加载纹理

```rust
use image::io::Reader as ImageReader;

let img = ImageReader::open("assets/sprite.png")?.decode()?;
let tex_info = d3d11_utils::load_texture_from_dynamic_image(&gfx.device, &img)?;

// 存储到纹理管理器
textures.insert("sprite".to_string(), Arc::new(tex_info));
```

⚠️ **Warning**：
- `create_texture_2d` 的 `cpu_access_flags` 参数类型是 `u32` 而不是 `D3D11_CPU_ACCESS_FLAG`，使用时需要注意类型转换
- `create_texture_2d` 不支持多 Mip Level 创建（`MipLevels: 1` 硬编码）
- `load_texture_from_dynamic_image` 对 RGB8 格式进行了 R8G8B8→R8G8B8A8 扩展（增加 alpha 通道为 255），这是 D3D11 的限制

---

### 4.11 Sprite2D / Sprite2DObject / Sprite2DBuffer / HaveID

**文件**：`sprite2d.rs`  
**公开导出**：`krjw_engine::Sprite2D`, `krjw_engine::Sprite2DObject`, `krjw_engine::Sprite2DBuffer`, `krjw_engine::HaveID`

描述 2D 精灵的核心类型，以及按 pipeline 排序的缓冲区。

#### HaveID

```rust
pub trait HaveID {
    fn get_id(&self) -> u64;   // 返回唯一标识符
}
```

用于在排序迭代中检测 pipeline（纹理/着色器）切换。

#### Sprite2D

```rust
pub struct Sprite2D {
    pub origin_px: Vec2,     // 原点/轴点（像素），例如 center = size_px * 0.5
    pub size_px: Vec2,       // 渲染尺寸（像素）
    pub uv_tl_px: Vec2,      // 纹理左上角 UV 坐标（像素）
    pub uv_size_px: Vec2,    // UV 矩形尺寸（像素）
}
```

**⚠ 关键注意**：所有值以**像素**为单位（非归一化 UV），因为 SpriteBatch2D 内部会自动根据纹理尺寸归一化。常见错误是用了 0~1 的归一化 UV 值。

例如，对于 512×512 的纹理，要选取左上角 64×64 的区域：
- ❌ 错误：`uv_tl_px: Vec2::new(0.0, 0.0), uv_size_px: Vec2::new(0.125, 0.125)`
- ✅ 正确：`uv_tl_px: Vec2::new(0.0, 0.0), uv_size_px: Vec2::new(64.0, 64.0)`

#### Sprite2DObject

```rust
pub struct Sprite2DObject<T: HaveID + Clone, U: Clone> {
    pub spr: Sprite2D,               // 精灵几何描述符
    pub color: [f32; 4],            // RGBA 颜色（预乘 alpha）
    pub transform: U,               // 每精灵变换（如 Transform2D）
    pub pipeline: T,                // 流水线引用（如 TextureInfoArced）
    pub layer: f64,                 // 排序层级（值越小越先绘制）
}
```

#### Sprite2DBuffer

```rust
pub struct Sprite2DBuffer<T: HaveID + Clone, U: Clone> {
    // ...
}

impl<T: HaveID + Clone, U: Clone> Sprite2DBuffer<T, U> {
    pub fn reserve(&mut self, additional: usize);
    pub fn len(&self) -> usize;
    pub fn push(&mut self, sprite: &Sprite2DObject<T, U>);  // O(1) 插入
    pub fn sort(&mut self);            // 惰性排序（自动调用）
    pub fn clear(&mut self);
    pub fn for_each_sorted<B, F, G>(
        &mut self,
        ex: &mut B,                    // 外部上下文
        on_pipeline_change: F,         // pipeline 变化时调用
        on_item: G,                    // 每个精灵调用一次
    );
}
```

排序逻辑：先按 `layer` 升序，再按 `pipeline.get_id()` 升序。

#### Example

```rust
// 准备精灵对象
let sprite_obj = Sprite2DObject {
    spr: Sprite2D {
        origin_px: Vec2::new(16.0, 16.0),  // 中心点为原点
        size_px: Vec2::new(32.0, 32.0),
        uv_tl_px: Vec2::ZERO,
        uv_size_px: Vec2::new(32.0, 32.0),
    },
    color: [1.0, 1.0, 1.0, 1.0],
    transform: Transform2D {
        pos: Vec2::new(100.0, 200.0),
        scale: Vec2::ONE,
        rot: 0.0,
    },
    pipeline: TextureInfoArced(texture.clone()),
    layer: 10.0,
};

// 推入缓冲区
sprite_buf.push(&sprite_obj);

// 排序 + 提交绘制（通过 SpriteBatch2D::push_buffered）
batch.push_buffered(&gfx, &vp, &mut sprite_buf, |t| (t.pos, t.scale, t.rot));
```

---

### 4.12 TextureInfoArced

**文件**：`lib.rs`  
**公开导出**：`krjw_engine::TextureInfoArced`

`Arc<TextureInfo>` 的包装器，同时实现 `HaveID` 和 `Pipeline` trait，用于在 `Sprite2DBuffer` 中使用。

```rust
#[derive(Debug, Clone)]
pub struct TextureInfoArced(pub Arc<TextureInfo>);
```

- `HaveID`：使用 `Arc<TextureInfo>` 的指针地址作为唯一 ID
- `Pipeline`：调用 `batch.set_texture(self.0.srv.clone(), self.0.width, self.0.height)`

#### Example

```rust
let tex_arced = TextureInfoArced(textures["player"].clone());

let obj = Sprite2DObject {
    spr: Sprite2D { /* ... */ },
    color: [1.0; 4],
    transform: Transform2D::IDENTITY,
    pipeline: tex_arced,      // 👈 这里使用
    layer: 0.0,
};
```

⚠️ **Warning**：`HaveID` 的实现基于指针地址。如果 `Arc` 被 clone 但指向同一个 `TextureInfo`，它们的 ID 相同（正确行为）。如果 `TextureInfo` 被释放后新分配的对象使用了同一块内存地址，理论上会导致 ID 冲突，但概率极低。

---

### 4.13 SpriteBatch2D & Pipeline Trait

**文件**：`graphic/d3d11/sprite_batch_2d.rs`  
**公开导出**：`krjw_engine::SpriteBatch2D`, `krjw_engine::graphic::d3d11::sprite_batch_2d::Pipeline`

基于 D3D11 的精灵批渲染器，使用 4 顶点 quad + 16-bit 索引。

#### Pipeline Trait

```rust
pub trait Pipeline: HaveID + Clone {
    fn apply_to_batch(&self, batch: &mut SpriteBatch2D);
}
```

`TextureInfoArced` 提供了 `Pipeline` 的标准实现。

#### 方法

```rust
impl SpriteBatch2D {
    pub fn new(device, capacity, vs, ps, input_layout) -> Result<Self>;
    pub fn set_texture(&mut self, srv, width, height);
    pub fn set_vertex_shader(&mut self, vs);
    pub fn set_pixel_shader(&mut self, ps);
    pub fn add(&mut self, pos: Vec2, scale: Vec2, rot: f32, sprite: &Sprite2D, color: [f32; 4]) -> Result<()>;
    pub fn set_mvp(&self, gfx: &D3D11, mvp: &glam::Mat4);
    pub fn submit_and_draw(&mut self, gfx: &D3D11) -> Result<()>;
    pub fn draw(&mut self, gfx: &D3D11, mvp: &glam::Mat4) -> Result<()>;
    pub fn clear_batch(&mut self);
    pub fn count(&self) -> usize;

    // 将 Sprite2DBuffer 按 pipeline 排序后压入 internal batch（不提交绘制）
    // ⚠ mvp 需通过 set_mvp 在外部提前设置，一次 DrawCall 只能有一个 MVP
    pub fn push_buffered<T, U>(
        &mut self,
        gfx: &D3D11,
        buf: &mut Sprite2DBuffer<T, U>,
        extract_transform: impl Fn(&U) -> (Vec2, Vec2, f32),
    ) where T: Pipeline, U: Clone;

    /// push_buffered + submit_and_draw + clear_batch + buf.clear()
    /// 便捷方法：压入批量精灵 → 提交 → 清空 batch 和 buffer。
    /// ⚠ mvp 需通过 set_mvp 在外部提前设置。
    pub fn draw_buffer_and_clear<T, U>(
        &mut self,
        gfx: &D3D11,
        buf: &mut Sprite2DBuffer<T, U>,
        extract_transform: impl Fn(&U) -> (Vec2, Vec2, f32),
    ) where T: Pipeline, U: Clone;
}
```

#### Example — 标准用法

```rust
// 设置 MVP 矩阵
let vp = camera.vp_matrix();
batch.set_mvp(&gfx, &vp.transpose());

// 绑定纹理并添加精灵
batch.set_texture(tex_info.srv.clone(), tex_info.width, tex_info.height);
batch.add(
    Vec2::new(100.0, 200.0),   // 位置
    Vec2::new(2.0, 2.0),       // 缩放
    0.5,                        // 旋转
    &Sprite2D {
        origin_px: Vec2::new(16.0, 16.0),
        size_px: Vec2::new(32.0, 32.0),
        uv_tl_px: Vec2::ZERO,
        uv_size_px: Vec2::new(32.0, 32.0),
    },
    [1.0, 1.0, 1.0, 1.0],
)?;

// 提交绘制
batch.submit_and_draw(&gfx)?;
batch.clear_batch();
```

#### Example — push_buffered 独立使用（推入后手动提交）

`push_buffered` **只推入不提交**，且**不接受 `vp` 参数**——MVP 须通过 `set_mvp` 在外部提前设置。  
这样你可以在推入精灵后插入其他操作（如文字精灵），再一次性提交，同时确保所有精灵共享同一个 MVP。

```rust
// 第0步：设置 MVP（在整个 batch 生命周期内固定）
let vp = camera.vp_matrix();
batch.set_mvp(&gfx, &vp.transpose());

// 第1步：将精灵按 pipeline 分组推入 internal batch
batch.push_buffered(
    &gfx,
    &mut sprite_buf,
    |t: &Transform2D| (t.pos, t.scale, t.rot),
);

// 第2步：在相同 pipeline 下插入文字精灵
atlas.render_layout(&layout, offset, origin, transform, color, layer, &mut text_buf);

// 第3步：上传 AtlasText 图集脏页到 GPU
atlas.upload(&gfx)?;

// 第4步：手动提交（sprite_buf 和 text_buf 的精灵被一起绘制）
batch.submit_and_draw(&gfx)?;

// 第5步：手动清空 internal batch
batch.clear_batch();
```

> 💡 **分离的好处**：`push_buffered` 只做推入不做提交，你可以合并多个 `Sprite2DBuffer`（如精灵和文字）到同一次 draw call 中。且所有精灵共享同一个 MVP 矩阵。

#### Example — draw_buffer_and_clear 一次性用法

如果你不需要在推入精灵和提交之间插入其他操作，直接用 `draw_buffer_and_clear`：

```rust
// 第0步：设置 MVP
batch.set_mvp(&gfx, &vp.transpose());
// 一次性推入 → 提交 → 清空 batch → 清空 buffer
batch.draw_buffer_and_clear(
    &gfx,
    &mut sprite_buf,
    |t: &Transform2D| (t.pos, t.scale, t.rot),
);
```

> 💡 **`draw_buffer_and_clear` 内部等价于**：
> ```rust
> batch.set_mvp(gfx, vp);
> batch.push_buffered(gfx, buf, extract);
> batch.submit_and_draw(gfx)?;
> batch.clear_batch();
> buf.clear();
> ```

#### ❌ 常见错误

| 错误 | 后果 | 正确做法 |
|------|------|---------|
| `batch.set_mvp(&gfx, &vp)` 忘记 `.transpose()` | 精灵旋转/缩放异常 | `batch.set_mvp(&gfx, &vp.transpose())` |
| `add()` 前未调用 `set_texture()` | 运行时 panic | 先调用 `set_texture()` 再 `add()` |
| `Sprite2D` 的 UV 用了 0~1 归一化值 | 纹理采样完全错误 | UV 用像素值，引擎内部自动归一化 |
| `push_buffered` 后忘了 `submit_and_draw()` | 画面空白 | push_buffered 不会提交，需要手动调用 submit_and_draw |
| `push_buffered` 后忘了 `clear_batch()` | 下一帧累积旧精灵 | push_buffered 后手动 clear_batch |
| `push_buffered` 的 extract_transform 返回错误类型 | 编译错误 | 返回 `(Vec2, Vec2, f32)`，即 `(pos, scale, rot)` |

⚠️ **Warning**：
- `capacity` 不能超过 `0xffff / 4 = 16383`（因为使用 16-bit 索引，每个 quad 用 4 个顶点）
- `set_mvp` 接收的矩阵需要是**转置后的**（因为 HLSL 使用列主序，而 glam 是行主序）
- `add()` 如果没有先调用 `set_texture()` 会 panic（`expect("No texture set")`）
- `push_buffered` 在每次 pipeline 切换时会调用 `submit_and_draw` + `clear_batch`，这意味着每切换一次就会产生一次 draw call

---

### 4.14 ShapeBatch2D

**文件**：`graphic/d3d11/shape_batch_2d.rs`  
**公开导出**：`krjw_engine::ShapeBatch2D`

形状批渲染器，用于绘制无纹理的线段、矩形、圆形、多边形。

#### 方法

```rust
impl ShapeBatch2D {
    pub fn new(device, capacity, vs, ps, input_layout) -> Result<Self>;

    // 无 UV 方法（适用于 ps_solid 纯色渲染，无需绑定纹理）
    pub fn add_rect_no_uv(&mut self, pos: Vec2, size: Vec2, origin_px: Vec2, rot: f32, color: [f32; 4]);
    ///   `origin_px` — 左上方对齐方式（像素），(0,0)=左上，(w/2,h/2)=中心，(w,h)=右下。
    pub fn add_circle_no_uv(&mut self, pos: Vec2, radius: f32, color: [f32; 4], segments: u32);
    pub fn add_square_line_no_uv(&mut self, from: Vec2, to: Vec2, thickness: f32, color: [f32; 4]);
    pub fn add_polygon_no_uv(&mut self, points: &[Vec2], color: [f32; 4]);

    // 带 UV 方法（需要通过 set_texture 绑定纹理）
    pub fn add_rect(&mut self, pos, size, origin_px, rot, uv_tl_px, uv_size_px, color);
    pub fn add_circle(&mut self, pos, radius, uv_tl_px, uv_size_px, color, segments);

    // 通用
    pub fn push(&mut self, vertices: &[ShapeVertex], tri_indicies: &[[u16; 3]])；
    pub fn push_with_transform_2d(&mut self, vertices: &[ShapeVertex], tri_indicies: &[[u16; 3]], pos: Vec2, scale: Vec2, rot: f32)；
    pub fn set_texture(&mut self, srv, width, height);
    pub fn set_mvp(&self, gfx: &D3D11, mvp: &glam::Mat4);
    pub fn submit_and_draw(&mut self, gfx: &D3D11) -> Result<()>;
    pub fn draw(&mut self, gfx: &D3D11, mvp: &glam::Mat4) -> Result<()>;
    pub fn clear_batch(&mut self);
    pub fn count(&self) -> usize;
}
```

#### ❌ 常见错误

| 错误 | 后果 | 正确做法 |
|------|------|---------|
| `add_rect_no_uv` 缺少 `origin_px` 参数 | 编译错误 | 传 5 个参数：`(pos, size, origin_px, rot, color)` |
| 用 `ps_tex_rgba_2d` 作为 ShapeBatch2D 的像素着色器 | 需要绑定纹理但没绑定会导致奇怪效果 | 纯形状用 `ps_solid_2d` |
| `submit_and_draw` 后忘记 `clear_batch()` | 下一帧形状不断累积 | 每次提交后调用 `clear_batch()` |
| `set_mvp` 后忘记在 draw 时传入矩阵 | 使用上次的 MVP 矩阵 | `sb.set_mvp(gfx, &vp)` + `sb.submit_and_draw(gfx)` |

#### Example

```rust
// 绘制矩形（origin_px=ZERO 左上对齐）
shape_batch.add_rect_no_uv(
    Vec2::new(100.0, 100.0),
    Vec2::new(50.0, 50.0),
    Vec2::ZERO,      // origin_px: (0,0)=左上，(w/2,h/2)=中心，(w,h)=右下
    0.0,
    [1.0, 0.0, 0.0, 1.0],
);

// 绘制线段
shape_batch.add_square_line_no_uv(
    Vec2::new(0.0, 0.0),
    Vec2::new(200.0, 0.0),
    2.0,
    [0.0, 1.0, 0.0, 1.0],
);

// 绘制圆形
shape_batch.add_circle_no_uv(
    Vec2::new(300.0, 300.0),
    50.0,
    [0.0, 0.0, 1.0, 0.5],
    32,
);

// 提交
let mvp = camera.vp_matrix().transpose();
shape_batch.draw(&gfx, &mvp)?;
shape_batch.clear_batch();
```

---

### 4.15 Transform2D

**文件**：`transform2d.rs`  
**公开导出**：`krjw_engine::Transform2D`

可组合的 2D 变换：旋转 → 缩放 → 平移（RST）。

#### 字段

```rust
pub struct Transform2D {
    pub pos: Vec2,    // 世界空间位置
    pub scale: Vec2,  // 局部缩放
    pub rot: f32,     // 旋转（弧度，逆时针）
}

impl Transform2D {
    pub const IDENTITY: Self;  // 单位变换
}
```

#### 方法

```rust
impl Transform2D {
    // Builder 模式
    pub fn with_pos(self, pos: Vec2) -> Self;
    pub fn with_scale(self, scale: Vec2) -> Self;
    pub fn with_rot(self, rot: f32) -> Self;
    pub fn move_by(self, pos: Vec2) -> Self;
    pub fn scale_by(self, scale: Vec2) -> Self;
    pub fn rotate_by(self, rot: f32) -> Self;

    // 组合与变换
    pub fn transform(&self, parent: &Transform2D) -> Self;  // 与父级组合
    pub fn transform_components(&self, pos: Vec2, scale: Vec2, rot: f32) -> Self;
    pub fn transform_point(&self, local_point: Vec2) -> Vec2;     // 局部→世界
    pub fn inverse_transform_point(&self, world_point: Vec2) -> Vec2;  // 世界→局部
}
```

#### 组合规则

对于子实体：
```
world = parent * self
```

数学表达式：
```
result.pos  = parent.pos + rotate(self.pos, parent.rot) * parent.scale
result.scale = self.scale * parent.scale
result.rot  = self.rot + parent.rot
```

#### Example

```rust
// 层级变换：角色 → 武器
let player_xform = Transform2D {
    pos: Vec2::new(100.0, 200.0),
    scale: Vec2::new(1.0, 1.0),
    rot: 0.5,  // 角色旋转
};

let weapon_local = Transform2D {
    pos: Vec2::new(30.0, 0.0),  // 武器相对角色偏移
    scale: Vec2::ONE,
    rot: 0.0,
};

let weapon_world = weapon_local.transform(&player_xform);
// weapon_world.pos = (100 + 30*cos(0.5), 200 + 30*sin(0.5))
```

```rust
// 点变换
let local_point = Vec2::new(10.0, 20.0);
let world_point = xform.transform_point(local_point);
let back_to_local = xform.inverse_transform_point(world_point);
```

---

### 4.16 Camera2D

**文件**：`camera2d.rs`  
**公开导出**：`krjw_engine::Camera2D`

正交 2D 相机，处理 View-Projection 矩阵和坐标转换。  

**⚠ 关键注意**：Camera2D 的视口坐标系统一为 X+ 为右、Y+ 为下，(0,0) 为视口**中心**。这与传统 2D 图形（Y+ 向上，原点在左上角）不同。

示意图：  
```
                        [T]                           Y-
   ┌─────────────────────┬───────────────────────┐    ↑
   │ [TL]                │                   [TR]│    ｜
   │                     │                       │    ｜
   │                     │                       │    ｜
   │                     │                       │    ｜
[L]├────────────────────[C]──────────────────────┤[R] Ｈ
   │                     │                       │    ｜
   │                     │                       │    ｜
   │                     │                       │    ｜
   │[BL]                 │                   [BR]│    ｜
   └─────────────────────┴───────────────────────┘    ↓
                        [B]                           Y+
  X-<────────────────────W───────────────────────> X+
```

**三层次坐标转换**：

```
Screen Space (像素)              View Space (归一化)            World Space (世界单位)
  (0,0) → (viewport_w, h)      (-1,-1) → (1,1)              camera.position ± viewport_size/2 * zoom
       │                              │                              │
       │  screen_to_world              │  view_matrix                 │
       │  (逆 projection + 逆 view)    │  (平移 + 旋转)               │
       ▼                              ▼                              ▼
  鼠标点击位置                     [-1, 1] 范围                   游戏世界坐标
```

- **projection_matrix**：将 `[-1, 1]` 的视口范围映射到实际的像素坐标
- **view_matrix**：将世界坐标平移到相机位置（加上旋转）
- **vp_matrix**：`projection_matrix * view_matrix`，直接用于渲染

**zoom 的数学含义**：
```
可见世界范围 width  = viewport_size.x * zoom.x
可见世界范围 height = viewport_size.y * zoom.y
```
即 `zoom = 1.0` 时，可见范围正好等于窗口大小（以像素为单位的世界单位）。`zoom = 2.0` 时可见范围扩大一倍。

#### 字段

```rust
pub struct Camera2D {
    pub position: Vec2,       // 世界空间中的相机位置
    pub rotation: f32,        // 相机旋转（弧度）
    pub zoom: Vec2,           // 缩放（Vec2 支持非均匀缩放）
    pub viewport_pos: Vec2,   // 视口左上角（窗口像素）
    pub viewport_size: Vec2,  // 视口尺寸（窗口像素）
}
```

#### 方法

```rust
impl Camera2D {
    pub fn new(window_size_px: Vec2) -> Self;  // 创建覆盖全窗口的相机
    pub fn move_by(&mut self, pos: Vec2);
    pub fn walk_xy(&mut self, xy: Vec2);              // 随旋转方向移动
    pub fn walk_xplus(&mut self, xplus: f32);         // 沿相机右方移动
    pub fn walk_yplus(&mut self, yplus: f32);         // 沿相机上方移动

    // 矩阵
    pub fn vp_matrix(&self) -> Mat4;     // 完整 View-Projection 矩阵
    pub fn view_matrix(&self) -> Mat4;
    pub fn projection_matrix(&self) -> Mat4;

    // 视口
    pub fn apply_viewport(&self, gfx: &D3D11);

    // 坐标转换
    pub fn screen_to_world(&self, screen_px: Vec2) -> Vec2;
    pub fn world_to_screen(&self, world_pos: Vec2) -> Vec2;
}
```

#### walk_xplus / walk_yplus 的数学推导

这两个方法实现了"沿相机朝向方向移动"：

```rust
pub fn walk_xplus(&mut self, xplus: f32) {
    // X+ 方向 = (cos(rot), sin(rot))，即旋转后的右方
    let (sin, cos) = self.rotation.sin_cos();
    self.position += Vec2::new(cos, sin) * xplus;
}

pub fn walk_yplus(&mut self, yplus: f32) {
    // Y+ 方向 = (-sin(rot), cos(rot))，即旋转后的上方（注意 Y+ 向下）
    let (sin, cos) = self.rotation.sin_cos();
    self.position += Vec2::new(-sin, cos) * yplus;
}
```

**示例**：如果相机旋转 90°（π/2），则 `walk_xplus` 实际会向上移动（因为右方变成了上方）。

#### Example

```rust
// 初始化
let mut camera = Camera2D::new(Vec2::new(960.0, 600.0));

// 每帧更新
// WASD 移动
let speed = 200.0 * dt;
if key_pressed!(driver, KeyCode::KeyW) {
    camera.walk_yplus(speed * dt);     // 沿相机上方向前
}
if key_pressed!(driver, KeyCode::KeyS) {
    camera.walk_yplus(-speed * dt);    // 沿相机下方向后
}
if key_pressed!(driver, KeyCode::KeyA) {
    camera.walk_xplus(-speed * dt);    // 沿相机左方
}
if key_pressed!(driver, KeyCode::KeyD) {
    camera.walk_xplus(speed * dt);     // 沿相机右方
}

// 滚轮缩放
let (_, wheel_y) = driver.mouse().get_mouse_wheel_delta();
camera.zoom *= 1.0 - (wheel_y as f32) * 0.1;

// 渲染时使用
let vp = camera.vp_matrix();
camera.apply_viewport(&gfx);
batch.set_mvp(&gfx, &vp.transpose());
// ... 绘制 ...

// 坐标转换
let world_click = camera.screen_to_world(mouse_pos);
let screen_enemy = camera.world_to_screen(enemy_pos);
```

#### ❌ 常见错误

| 错误 | 后果 | 正确做法 |
|------|------|---------|
| 假设 Y+ 向上 | 画面上下颠倒 | Y+ 向下，(0,0) 在视口中心 |
| 假设 (0,0) 是左上角 | 坐标偏移，物体出现在错误位置 | 屏幕中心是 (0,0)，左上角是 `(-w/2, -h/2)` |
| HUD 使用 `camera.vp_matrix()` | HUD 随相机移动而漂移 | HUD 使用 `Mat4::orthographic_lh(0, w, h, 0, ...)` 屏幕空间矩阵 |
| `screen_to_world` 忘了调用的 | 鼠标碰撞检测在错误位置 | 先 `camera.screen_to_world(mouse_pos)` 再检测 |
| `apply_viewport` 忘了调用 | 视口未设置，渲染可能异常 | 渲染前调用 `camera.apply_viewport(&gfx)` |

⚠️ **Warning**：
- `apply_viewport` 直接与 D3D11 耦合
- `vp_matrix()` 返回的矩阵需要**转置**后才能传入 `set_mvp`（`batch.set_mvp(gfx, &vp.transpose())`）

---

### 4.17 Collider / ColliderInstance / Overlap

**文件**：`collider.rs`  
**公开导出**：`krjw_engine::Collider`, `krjw_engine::ColliderInstance`, `krjw_engine::Overlap`

碰撞形状与碰撞检测系统。

#### Collider

```rust
pub enum Collider {
    AABB { half_size: Vec2 },     // 轴对齐包围盒（忽略旋转）
    Rect { half_size: Vec2 },     // 有向包围盒（应用完整变换）
    Circle { radius: f32 },       // 圆形（旋转无关）
}
```

#### ColliderInstance

```rust
pub struct ColliderInstance<'a> {
    pub shape: &'a Collider,
    pub xform: Transform2D,
}
```

#### Overlap

```rust
pub struct Overlap {
    pub hit: bool,       // 是否碰撞
    pub push: Vec2,      // 将自身推开的分辨向量
    pub normal: Vec2,    // 碰撞法线（从自身指向对方）
    pub depth: f32,      // 穿透深度
}
```

#### 方法

```rust
impl<'a> ColliderInstance<'a> {
    pub fn new(shape: &'a Collider, xform: Transform2D, parent: Option<&Transform2D>) -> Self;
    pub fn apply_transform(&self, parent: &Transform2D) -> Self;
    pub fn contains_point(&self, point: Vec2) -> bool;    // 点包含检测
    pub fn overlaps(&self, other: &ColliderInstance) -> Overlap;  // 碰撞检测
}
```

#### 碰撞配对算法

| 配对 | 算法 |
|------|------|
| AABB × AABB | 分离轴（简化为轴对齐比较） |
| AABB × Circle | 最近点法 |
| Circle × Circle | 球心距离 |
| 包含 Rect 的组合 | SAT（分离轴定理，最多测试 4 条轴） |

#### Example

```rust
let player_shape = Collider::Rect { half_size: Vec2::new(16.0, 16.0) };
let player_inst = ColliderInstance::new(
    &player_shape,
    Transform2D { pos: Vec2::new(100.0, 200.0), scale: Vec2::ONE, rot: 0.0 },
    None,
);

let wall_shape = Collider::AABB { half_size: Vec2::new(200.0, 20.0) };
let wall_inst = ColliderInstance::new(
    &wall_shape,
    Transform2D { pos: Vec2::new(0.0, 300.0), scale: Vec2::ONE, rot: 0.0 },
    None,
);

let overlap = player_inst.overlaps(&wall_inst);
if overlap.hit {
    println!("Collision! Push: {:?}, Normal: {:?}, Depth: {}", 
             overlap.push, overlap.normal, overlap.depth);
    // 使用 push 向量将玩家推开
    player.xform.pos += overlap.push;
}
```

#### ❌ 常见错误

| 错误 | 后果 | 正确做法 |
|------|------|---------|
| `Collider::AABB` 假设包含旋转 | 碰撞检测不准确 | AABB 忽略旋转，用 `Rect` 替代 |
| 忘记使用 `screen_to_world` | 点击检测位置偏移 | 先 `camera.screen_to_world(mouse)` 再传递给 `contains_point` |
| 未使用 `ColliderInstance::new` | 可能忘记应用父变换 | 使用构造方法可传入 `parent` 参数 |

---

### 4.18 AtlasText & TextLayout

**文件**：`atlas_text.rs`  
**公开导出**：`krjw_engine::AtlasText`, `krjw_engine::TextLayout`

动态文字图集系统。使用 `cosmic-text` 排版，`swash` 直接光栅化，自定义 Skyline Packer 打包到 2048×2048 的图集页中。

#### TextLayout

```rust
pub struct TextLayout {
    pub(crate) glyphs: Vec<(cosmic_text::CacheKey, Vec2)>,
    pub content_size: Vec2,    // 文本的像素尺寸
}
```

存储排版结果，可以重复使用以在不同变换/颜色下渲染相同文字（例如绘制阴影效果）。

#### AtlasText

```rust
pub struct AtlasText {
    // ...
    pub lifetime_a: f32,  // 字形缓存的寿命参数 a
    pub lifetime_b: f32,  // 字形缓存的寿命参数 b
}
```

#### 方法

```rust
impl AtlasText {
    pub fn new(device: &ID3D11Device, lifetime_a: f32, lifetime_b: f32) -> Result<Self>;

    // 信息
    pub fn page_count(&self) -> usize;
    pub fn texture_info(&self, page_idx: usize) -> &TextureInfo;
    pub fn texture_arced(&self, page_idx: usize) -> TextureInfoArced;
    pub fn set_lifetime_params(&mut self, a: f32, b: f32);

    // 排版与渲染
    pub fn layout_text(&mut self, text: &str, metrics: Metrics, attrs: Attrs,
                       device: &ID3D11Device) -> Result<TextLayout>;
    /// `render_layout` 的 `origin` 参数：`Vec2::ZERO` = 左上对齐，
    /// `Vec2::new(layout.content_size.x, 0.0)` = 右上对齐，
    /// `layout.content_size * 0.5` = 居中。
    pub fn render_layout(&self, layout: &TextLayout, offset: Vec2, origin: Vec2,
                         transform: Transform2D, color: [f32; 4], layer: f64,
                         buffer: &mut Sprite2DBuffer<TextureInfoArced, Transform2D>);
    pub fn render_layout_simple(&self, layout: &TextLayout, offset: Vec2,
                                color: [f32; 4], layer: f64,
                                buffer: &mut Sprite2DBuffer<TextureInfoArced, Transform2D>);
    pub fn render_text(&mut self, text: &str, metrics: Metrics, attrs: Attrs,
                       offset: Vec2, color: [f32; 4], layer: f64,
                       buffer: &mut Sprite2DBuffer<TextureInfoArced, Transform2D>,
                       device: &ID3D11Device) -> Result<()>;

    // GPU 上传（每帧需调用）
    pub fn upload(&mut self, gfx: &D3D11) -> Result<()>;
    pub fn clear(&mut self, device: &ID3D11Device) -> Result<()>;
}
```

#### 字形缓存寿命

寿命公式：`lifetime = max(side * lifetime_a + lifetime_b, 1)`（帧数）
- `side` = 字形的最大边长
- `lifetime_a`：基于大小的系数（大字形活更短）。**可以为负数**，例如 `-50.0` 表示大字形寿命更短
- `lifetime_b`：固定寿命偏移

#### ❌ 常见错误

| 错误 | 后果 | 正确做法 |
|------|------|---------|
| `render_layout` 前忘了 `layout_text` | 空布局，无文字显示 | 先 `layout_text`，再 `render_layout` |
| 忘了调用 `upload()` | 文字不会更新在屏幕上 | 在 `push_buffered` 前调用 `atlas.upload(&gfx)?` |
| `push_buffered` 在 `upload` 之前 | 文字使用旧纹理数据 | 顺序：`push` → `upload` → `push_buffered` |
| 每帧都 `layout_text` 不变的字符串 | 性能浪费 | 不变的文字缓存 TextLayout，重复使用 |
| 不知道 `origin` 参数的作用 | 文字对齐混乱 | `Vec2::ZERO`=左上, `content_size*0.5`=居中 |

#### Example

```rust
// 初始化
let mut atlas = AtlasText::new(&gfx.device, -20.0, 12000.0)?; // 大字形约 12000 帧，小字形更久

// 每帧流程
// 1. 排版（首次或文字变化时）
let layout = atlas.layout_text(
    "Hello, World!",
    Metrics::new(14.0, 20.0),     // font_size, line_height
    Attrs::new(),
    &gfx.device,
)?;

// 2. 渲染到精灵缓冲区
atlas.render_layout_simple(&layout, Vec2::new(10.0, 10.0), [1.0, 1.0, 1.0, 1.0], 100.0, &mut text_buf);

// 3. 上传图集脏页到 GPU
atlas.upload(&gfx)?;

// 4. 渲染精灵缓冲区（通过 SpriteBatch2D 提交）
let vp = camera.vp_matrix();
batch.push_buffered(&gfx, &vp.transpose(), &mut text_buf, |t| (t.pos, t.scale, t.rot));
```

⚠️ **Warning**：
- **图集页不可回收**：当字形被逐出（evict）后，其在 atlas 中占用的空间无法回收利用。
- 每帧调用 `upload()` 时会**整页上传**，即使只更改了几个像素
- 没有内置字体管理——所有字体通过 `FontSystem` 自动加载系统字体，无法指定自定义字体文件路径

---

## 5. 完整示例

以下是一个综合示例，展示如何使用引擎的大部分核心功能。更详细的实战教程请参见[第 7 节](#7-详细使用教程)。

**`my_app/src/main.rs`**
```rust
mod app;

use krjw_engine::EngineHandler;
use winit::event_loop::ControlFlow;

fn main() {
    let event_loop = winit::event_loop::EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut handler = EngineHandler::new(|window, hwnd, rx| {
        let mut app = app::App::default();
        app.run(window, hwnd, rx)
    });
    event_loop.run_app(&mut handler).unwrap();
}
```

**`my_app/src/app.rs`**
```rust
use std::sync::mpsc::Receiver;
use std::sync::Arc;
use anyhow::Result;
use glam::Vec2;
use image::io::Reader as ImageReader;
use krjw_engine::*;
use cosmic_text::{Attrs, Metrics, Shaping};
use winit::keyboard::KeyCode;

pub struct App {
    pub ctx: Option<AppContext>,
}

pub struct AppContext {
    pub window: winit::window::Window,
    pub gfx: D3D11,
    pub batch: SpriteBatch2D,
    pub shape_batch: ShapeBatch2D,
    pub textures: std::collections::HashMap<String, Arc<TextureInfo>>,
    pub camera: Camera2D,
    pub timer: Timer,
    pub sprite_buf: Sprite2DBuffer<TextureInfoArced, Transform2D>,
    pub text_buf: Sprite2DBuffer<TextureInfoArced, Transform2D>,
    pub atlas: AtlasText,
    pub player_pos: Vec2,
    pub player_rot: f32,
}

impl Default for App {
    fn default() -> Self { App { ctx: None } }
}

impl App {
    pub fn run(&mut self, window: winit::window::Window, hwnd: isize,
               rx: Receiver<AppMsg>) -> Result<()> {
        let gfx = D3D11::init_on_hwnd(hwnd)?;
        let size = window.inner_size();
        let window_size = Vec2::new(size.width as f32, size.height as f32);

        let mut driver = EventDriver::new(rx);
        driver.set_initial_window_size(size.width, size.height);

        let batch = SpriteBatch2D::new(&gfx.device, 2048,
            &gfx.states.vs_puc_m_2d, &gfx.states.ps_tex_rgba_2d,
            &gfx.states.input_layout_puc)?;
        let shape_batch = ShapeBatch2D::new(&gfx.device, 4096,
            &gfx.states.vs_puc_m_2d, &gfx.states.ps_solid_2d,
            &gfx.states.input_layout_puc)?;

        let mut textures = std::collections::HashMap::new();
        if let Ok(img) = image::io::Reader::open("assets/player.png") {
            if let Ok(img) = img.decode() {
                let tex = d3d11_utils::load_texture_from_dynamic_image(&gfx.device, &img)?;
                textures.insert("player".to_string(), Arc::new(tex));
            }
        }

        let camera = Camera2D::new(window_size);
        let atlas = AtlasText::new(&gfx.device, 0.5, 60.0)?;

        let ctx = AppContext {
            window, gfx, batch, shape_batch, textures,
            camera, timer: Timer::default(),
            sprite_buf: Sprite2DBuffer::default(),
            text_buf: Sprite2DBuffer::default(),
            atlas, player_pos: Vec2::ZERO, player_rot: 0.0,
        };
        self.ctx = Some(ctx);
        self.main_loop(&mut driver)
    }

    fn main_loop(&mut self, driver: &mut EventDriver) -> Result<()> {
        let ctx = self.ctx.as_mut().unwrap();

        loop {
            let events = driver.poll_frame();
            if events.close_requested || events.disconnected { break; }

            driver.if_window_size_dirty(|w, h| {
                ctx.gfx.on_resize(w, h)?;
                ctx.camera.viewport_size = Vec2::new(w as f32, h as f32);
                Ok(())
            })?;

            let dt = ctx.timer.pre_frame_and_get_delta_time().min(0.05);

            self.update(ctx, driver, dt)?;
            self.render(ctx, dt)?;

            ctx.gfx.present()?;
            ctx.timer.post_frame_fpsc(dt);
            driver.end_frame();
        }
        Ok(())
    }

    fn update(&mut self, ctx: &mut AppContext, driver: &EventDriver, dt: f64) -> Result<()> {
        let speed = 200.0;
        if driver.keyboard().get_key_state(KeyCode::KeyW).is_pressed() {
            ctx.player_pos.y -= speed * dt as f32;
        }
        if driver.keyboard().get_key_state(KeyCode::KeyS).is_pressed() {
            ctx.player_pos.y += speed * dt as f32;
        }
        if driver.keyboard().get_key_state(KeyCode::KeyA).is_pressed() {
            ctx.player_pos.x -= speed * dt as f32;
        }
        if driver.keyboard().get_key_state(KeyCode::KeyD).is_pressed() {
            ctx.player_pos.x += speed * dt as f32;
        }

        Ok(())
    }

    fn render(&mut self, ctx: &mut AppContext, dt: f64) -> Result<()> {
        ctx.sprite_buf.clear();
        ctx.text_buf.clear();
        ctx.shape_batch.clear_batch();
        ctx.gfx.clear_screen(&[0.1, 0.1, 0.2, 1.0]);

        if let Some(tex) = ctx.textures.get("player") {
            let tex_arced = TextureInfoArced(tex.clone());
            let sprite_obj = Sprite2DObject {
                spr: Sprite2D {
                    origin_px: Vec2::new(32.0, 32.0),
                    size_px: Vec2::new(64.0, 64.0),
                    uv_tl_px: Vec2::ZERO,
                    uv_size_px: Vec2::new(64.0, 64.0),
                },
                color: [1.0, 1.0, 1.0, 1.0],
                transform: Transform2D {
                    pos: ctx.player_pos, scale: Vec2::ONE, rot: ctx.player_rot,
                },
                pipeline: tex_arced, layer: 10.0,
            };
            ctx.sprite_buf.push(&sprite_obj);
        }

        let vp = ctx.camera.vp_matrix().transpose();
        ctx.batch.push_buffered(&ctx.gfx, &vp, &mut ctx.sprite_buf, |t| (t.pos, t.scale, t.rot));
        ctx.batch.submit_and_draw(&ctx.gfx);

        Ok(())
    }
}
```

---

## 6. 常见陷阱与易错点

本章总结了使用引擎时最容易犯的错误。⚠️ 这个章节对 AI 模型（如 ChatGPT、Claude、DeepSeek 等）生成代码时的提示至关重要——如果你让 AI 帮你写代码，请确保 AI 了解这些注意事项。

### 6.1 全局性陷阱

| # | 陷阱 | ❌ 错误代码 | ✅ 正确代码 | 后果 |
|---|------|-----------|-----------|------|
| 1 | **MVP 矩阵忘记转置** | `batch.set_mvp(&gfx, &vp)` | `batch.set_mvp(&gfx, &vp.transpose())` | 精灵倾斜/方向错误 |
| 2 | **`add_rect_no_uv` 参数错误**（旧 API 只有 4 个参数） | `shape_batch.add_rect_no_uv(pos, size, rot, color)` | `shape_batch.add_rect_no_uv(pos, size, origin_px, rot, color)` | 编译错误 |
| 3 | **ShapeBatch2D 用错着色器** | `ShapeBatch2D::new(..., &ps_tex_rgba_2d, ...)` | `ShapeBatch2D::new(..., &ps_solid_2d, ...)` | 纯色形状需要纹理才能显示 |
| 4 | **SpriteBatch2D 用对着色器但忘了 set_texture** | `batch.add(...)` 前没有 `set_texture` | `batch.set_texture(srv, w, h)` 再 `batch.add(...)` | 运行时 panic |
| 5 | **`clear_batch` 忘记调用** | `submit_and_draw` 后直接下一帧 | `submit_and_draw` → `clear_batch` | 精灵/形状不断累积 |

### 6.2 坐标系统陷阱

| # | 陷阱 | 说明 |
|---|------|------|
| 6 | **Camera2D 的 Y+ 向下** | 传统图形学 Y+ 向上，但本引擎 Y+ 向下。如果从其他地方复制代码，很可能 Y 轴方向是反的 |
| 7 | **Camera2D 的 (0,0) 在视口中心** | 窗口左上角是 `(-viewport_size.x/2, -viewport_size.y/2)`，不是 `(0,0)` |
| 8 | **`screen_to_world` 必须调用** | 鼠标坐标是屏幕空间（像素），传递给碰撞检测前需要转换为世界坐标 |
| 9 | **HUD 用世界空间矩阵** | HUD 应该用屏幕空间正交矩阵，否则会随相机移动而漂移（除非特别需要） |

**屏幕空间 HUD 矩阵的正确写法**：
```rust
let (w, h) = driver.window_size();
let hud_mvp = glam::Mat4::orthographic_lh(0.0, w, h, 0.0, 0.0, 1.0);
let hud_vp = hud_mvp.transpose();
batch.push_buffered(gfx, &hud_vp, &mut buf, |x| (x.pos, x.scale, x.rot));
```

### 6.3 文字渲染陷阱

| # | 陷阱 | 说明 |
|---|------|------|
| 10 | **`upload()` 必须在 `push_buffered` 之前** | 正确的顺序是：`push()` / `render_layout()` → `upload()` → `push_buffered()` |
| 11 | **每帧对不变文字重复 `layout_text`** | 不变的字符串应该缓存 `TextLayout`，每帧复用 |
| 12 | **忽略 `origin` 参数** | `render_layout` 的 `origin` 控制对齐方式，`Vec2::ZERO`=左上，`content_size*0.5`=居中 |

### 6.4 纹理与精灵陷阱

| # | 陷阱 | 说明 |
|---|------|------|
| 13 | **`Sprite2D` 的 UV 用归一化值 (0~1)** | `uv_tl_px` 和 `uv_size_px` 都是用**像素**单位，引擎内部自动归一化 |
| 14 | **直接加载 RGB8 图片** | 引擎会将其扩展为 RGBA8（增加 alpha=255），这是 D3D11 的限制 |
| 15 | **`include_bytes!` 路径错误** | 路径相对于当前源文件（`file!()`），而不是相对于工作目录 |

### 6.5 帧循环陷阱

| # | 陷阱 | 说明 |
|---|------|------|
| 16 | **`end_frame()` 在 `present()` 之前调用** | 边缘状态会在同一帧被清除，导致后续输入检测失效 |
| 17 | **窗口 resize 后不更新 `camera.viewport_size`** | 相机视口与窗口不一致会导致画面拉伸 |
| 18 | **窗口 resize 后不调用 `gfx.on_resize()`** | 交换链的 back buffer 尺寸不匹配，可能导致渲染异常或黑屏 |

### 6.6 帧循环正确模板

```rust
loop {
    // 1. 轮询事件（必须在最前面）
    let events = driver.poll_frame();
    if events.close_requested || events.disconnected { break; }

    // 2. 处理窗口 resize
    driver.if_window_size_dirty(|w, h| {
        gfx.on_resize(w, h)?;
        camera.viewport_size = Vec2::new(w as f32, h as f32);
        Ok(())
    })?;

    // 3. Delta Time
    let dt = timer.pre_frame_and_get_delta_time().min(0.05);

    // 4. 更新逻辑
    self.update(...)?;

    // 5. 渲染
    //    a) 清空缓冲区/清屏
    //    b) 添加精灵/形状到缓冲区
    //    c) upload() AtlasText 脏页
    //    d) push_buffered() / submit_and_draw()

    // 6. Present（必须6在7之前）
    gfx.present()?;

    // 7. 帧结束处理（必须在6之后）
    timer.post_frame_fpsc(dt);
    driver.end_frame();
}
```

---

## 7. 详细使用教程

本章通过三个实际示例应用来演示引擎的各种使用模式，从简单到复杂逐步深入。每个示例都位于 workspace 中的独立 crate 下。

> ⚠️ **对读者和 AI 的提示**：
> 在阅读下面的示例后，如果让 AI（如 ChatGPT、Claude 等）生成新代码，请要求 AI **明确注意以下引擎特殊性**：
> 1. Camera2D 的坐标系统：Y+ 向下，(0,0) 在视口中心
> 2. MVP 矩阵需要 `.transpose()` 后才能传入 `set_mvp`
> 3. `Sprite2D` 的 UV 用像素坐标（非归一化 0~1）
> 4. `add_rect_no_uv` 需要 5 个参数（包含 `origin_px`）
> 5. ShapeBatch2D 用 `ps_solid_2d`，SpriteBatch2D 用 `ps_tex_rgba_2d`
> 6. AtlasText 的 `upload()` 必须在 `push_buffered` 之前调用

### 7.1 `app_shapes` —— 纯形状渲染入门

**路径**：`app_shapes/src/app.rs`  
**学习目标**：
- 理解最小化的 App 结构（无纹理、无文字、无音频）
- 掌握 `ShapeBatch2D` 绘制圆、线段、网格
- 理解相机控制和坐标转换

> ⚠️ **AI 注意**：本示例的相机控制和形状绘制是后续所有项目的基础。请特别注意 `camera.walk_xplus`/`walk_yplus` 的行为和 `ShapeBatch2D` 的 origin_px 参数。

#### 7.1.1 项目结构

`app_shapes` 是一个非常轻量的示例，仅依赖 `krjw_engine` 和 `glam`。它**不**使用纹理、音频或文字系统，完全通过 `ShapeBatch2D` 渲染。

**main.rs**（标准模板）：
```rust
fn main() {
    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut handler = EngineHandler::new(
        WindowAttributes::default()
            .with_title("KrisuRJW - Shapes")
            .with_inner_size(winit::dpi::Size::Logical(LogicalSize { width: 960.0, height: 600.0 })),
        |window, hwnd, rx| {
            let mut app = app::App::new();
            app.run(window, hwnd, rx)
        }
    );
    event_loop.run_app(&mut handler).unwrap();
}
```

#### 7.1.2 应用状态

```rust
pub struct App {
    pub ctx: Option<AppContext>,
    pub timer: Timer,
    pub balls: Vec<Ball>,         // 80 个弹球
    pub trails: Vec<(Vec2, [f32; 4])>,  // 拖尾效果
    pub mouse_pos: Vec2,
    pub mouse_down: bool,
    pub attract_mode: bool,       // 吸引/爆炸模式切换
}

pub struct AppContext {
    pub window: Window,
    pub gfx: D3D11,
    pub batch: SpriteBatch2D,     // 创建但未使用
    pub shape_batch: ShapeBatch2D,
    pub camera: Camera2D,
}
```

> 💡 **关键设计**：虽然 `SpriteBatch2D` 被创建了，但由于不渲染任何纹理精灵，它实际上未被使用。`ShapeBatch2D` 才是唯一的渲染器。

#### 7.1.3 Ball 结构体

```rust
pub struct Ball {
    pos: Vec2,
    vel: Vec2,
    color: [f32; 4],
    radius: f32,
}
```

80 个 ball 使用**黄金角度**（137.508°）分布在圆周上，确保视觉均匀分布：
```rust
let angle = i as f32 * 137.508_f32.to_radians(); // 黄金角度
let radius = 200.0 + rng.next() * 300.0;
```

#### 7.1.4 帧循环

```rust
loop {
    let events = driver.poll_frame();
    if events.close_requested || events.disconnected { break; }

    // 窗口大小变化
    driver.if_window_size_dirty(|w, h| {
        ctx.gfx.on_resize(w, h)?;
        ctx.camera.viewport_size = Vec2::new(w as f32, h as f32);
        Ok(())
    })?;

    let dt = self.timer.pre_frame_and_get_delta_time() as f64;

    self.handle_input(&driver, dt32);  // 输入处理
    self.update_balls(dt);             // 物理更新
    self.render_frame(&driver)?;       // 渲染

    ctx.gfx.present()?;
    self.timer.post_frame_fpsc(dt);
    driver.end_frame();
}
```

#### 7.1.5 输入处理

```rust
fn handle_input(&mut self, driver: &EventDriver, dt: f32) {
    let camera = &mut ctx.camera;

    // 相机移动速度随缩放调整
    let move_speed = 600.0 * (1.0 / camera.zoom.x.max(0.01));

    // 相机控制
    let k = |code| driver.keyboard().get_key_state(code).is_pressed();
    if k(KeyCode::KeyQ) { camera.rotation -= 2.0 * dt; }
    if k(KeyCode::KeyE) { camera.rotation += 2.0 * dt; }
    if k(KeyCode::KeyA) { camera.walk_xplus(-move_speed * dt); }
    if k(KeyCode::KeyD) { camera.walk_xplus(move_speed * dt); }
    if k(KeyCode::KeyW) { camera.walk_yplus(-move_speed * dt); }
    if k(KeyCode::KeyS) { camera.walk_yplus(move_speed * dt); }

    // 滚轮缩放
    if let Some(pixel) = driver.mouse().get_pixel_wheel() {
        // 触控板像素滚轮
        camera.zoom *= 1.05_f32.powf(dt as f32 * pixel.1.abs() as f32);
    } else {
        // 普通鼠标行滚轮
        let wheel = driver.mouse().get_mouse_wheel_delta();
        if wheel.1 > 0.0 { camera.zoom *= 2.0_f32.powf(dt as f32); }
        if wheel.1 < 0.0 { camera.zoom /= 2.0_f32.powf(dt as f32); }
    }

    // 模式切换
    if driver.keyboard().get_key_state(KeyCode::Space).is_down_true_edge() {
        self.attract_mode = !self.attract_mode;
    }
    // 重置
    if driver.keyboard().get_key_state(KeyCode::KeyR).is_down_true_edge() {
        // 重新生成所有 ball 的位置和速度
    }
}
```

> 💡 **关键技巧**：
> - `walk_xplus`/`walk_yplus` 是随相机旋转方向移动的方法，实现了"前进方向随相机旋转"的行为
> - `get_pixel_wheel` 和 `get_mouse_wheel_delta` 分别对应触控板像素滚轮和普通鼠标滚轮，优先检测像素滚轮
> - `is_down_true_edge()` 确保按一次 Space/R 只触发一次，而非连续触发

#### 7.1.6 物理更新

```rust
fn update_balls(&mut self, dt: f64) {
    let world_mouse = camera.screen_to_world(self.mouse_pos);

    // 计算受力中心
    let mut force_center = Vec2::ZERO;
    if self.mouse_down {
        force_center = world_mouse;  // 鼠标按下时吸引到鼠标位置
    } else if self.attract_mode {
        force_center = Vec2::ZERO;   // 吸引到原点
    }

    for ball in &mut self.balls {
        let to_center = force_center - ball.pos;
        let dist = to_center.length().max(1.0);

        if self.mouse_down || self.attract_mode {
            ball.vel += to_center / dist * strength * dt;  // 向心力
        } else {
            ball.vel += to_center / dist * -300.0 * dt;    // 爆炸：反向力
        }

        ball.vel *= (1.0 - 0.5 * dt).max(0.0);  // 阻力衰减
        ball.pos += ball.vel * dt;

        // 添加拖尾
        self.trails.push((ball.pos, ball.color));
    }

    // 拖尾透明度衰减
    for trail in &mut self.trails { trail.1[3] *= 0.97; }
    self.trails.retain(|t| t.1[3] > 0.01);
}
```

> 💡 **关键设计**：拖尾系统通过保留历史帧位置并逐渐降低 alpha 实现。通过 `retain` 过滤掉已经完全透明的拖尾，避免无限增长。

#### 7.1.7 渲染（纯形状）

```rust
fn render_frame(&mut self, _driver: &EventDriver) -> Result<()> {
    ctx.gfx.clear_screen(&[0.05, 0.05, 0.08, 1.0]);
    let vp = ctx.camera.vp_matrix().transpose();

    // 1. 绘制网格
    draw_grid(sb, camera);

    // 2. 绘制拖尾（小圆点）
    for (pos, color) in &trails {
        sb.add_circle_no_uv(*pos, 1.5, *color, 8);
    }

    // 3. 绘制 ball
    for ball in &balls {
        sb.add_circle_no_uv(ball.pos, ball.radius * 2.5, ..., 20);  // 光晕
        sb.add_circle_no_uv(ball.pos, ball.radius, ball.color, 20);  // 本体
        sb.add_circle_no_uv(ball.pos, ball.radius + 1.0, ..., 20);  // 边缘高光
    }

    // 4. 绘制鼠标十字准星
    sb.add_square_line_no_uv(world_mouse + Vec2::new(-cross_size, 0.0), ...);
    sb.add_square_line_no_uv(world_mouse + Vec2::new(0.0, -cross_size), ...);

    sb.set_mvp(gfx, &vp);
    sb.submit_and_draw(gfx)?;
}
```

> 💡 **渲染层次**：每层使用不同的绘制方法，但都提交到同一个 `ShapeBatch2D`，最后一次性提交。注意每个 ball 绘制了 3 个圆：外层光晕（大半径低 alpha）、本体、边缘高光（小半径白边）。这展示了利用 alpha 叠加实现视觉层次感。

#### 7.1.8 网格绘制

```rust
fn draw_grid(sb: &mut ShapeBatch2D, camera: &Camera2D) {
    let spacing = 100.0;
    let hw = camera.viewport_size.x * 0.5 * camera.zoom.x;
    let hh = camera.viewport_size.y * 0.5 * camera.zoom.y;
    let half_side = (hw * hw + hh * hh).sqrt().max(spacing); // 半对角线长度
    let cx = camera.position.x;
    let cy = camera.position.y;

    // 计算需要绘制的范围
    let min_x = ((cx - half_side) / spacing).floor() * spacing;
    let max_x = ((cx + half_side) / spacing).ceil() * spacing;
    let min_y = ((cy - half_side) / spacing).floor() * spacing;
    let max_y = ((cy + half_side) / spacing).ceil() * spacing;

    // 绘制垂直线和水平线
    let mut x = min_x;
    while x <= max_x && count < 200 {
        sb.add_square_line_no_uv(Vec2::new(x, min_y), Vec2::new(x, max_y), 1.0, ...);
        x += spacing;
    }
    // ... 水平线同理
}
```

> 💡 **技巧**：网格只绘制在屏幕可见范围内的线，而不是绘制一个巨大的静态网格。通过 `camera.position` 和 `viewport_size * zoom` 计算可见范围，裁剪掉看不见的网格线，提高性能。

---

### 7.2 `app_sethsweeper` —— 纹理精灵+文字+碰撞综合示例

**路径**：`app_sethsweeper/src/app.rs`  
**学习目标**：
- 使用 `SpriteBatch2D` 渲染纹理精灵（从 PNG 图片切片）
- 使用 `AtlasText` 渲染 HUD 文字（带阴影）
- 使用 `ColliderInstance` 进行点碰撞检测（鼠标悬停）
- 使用 `ShapeBatch2D` 绘制碰撞体轮廓和网格
- 集成 `kira` 音频系统
- 使用 `push_buffered` 的 Shadow 模式

> ⚠️ **AI 注意**：本示例引入了纹理加载、音频和文字渲染。请特别注意 HUD 文字要用屏幕空间矩阵而非相机矩阵，以及 `upload()` 和 `push_buffered` 的调用顺序。

#### 7.2.1 应用结构

`app_sethsweeper` 是一个综合性的示例，展示了一个"赛博吸尘器"效果：

- 显示一个 12×9 的 Seth 头像瓷砖网格（从 `seth.png` 纹理图集切片）
- 每个瓷砖都有独立的物理（位置、速度、旋转）
- 鼠标左键吸引瓷砖向鼠标位置移动
- X 键强力制动
- 鼠标悬停在瓷砖上时碰撞体高亮

#### 7.2.2 纹理加载

```rust
fn init_textures(gfx: &D3D11) -> Result<HashMap<String, Arc<TextureInfo>>> {
    let mut textures = HashMap::new();

    // 使用 include_bytes! 在编译时将图片嵌入到二进制文件中
    // 这样运行时不需要额外的文件加载
    let img = image::load_from_memory(include_bytes!("../seth.png"))?;
    let tex_info = load_texture_from_dynamic_image(&gfx.device, &img)?;
    textures.insert("seth".to_string(), Arc::new(tex_info));

    Ok(textures)
}
```

**纹理图集切片**：`seth.png` 是一个包含 12×9 个单元格的图集，每个单元格代表一帧动画或一个不同的表情。通过计算每个单元格的 UV 位置来实现切片：

```rust
let cell_w = texture.width as f32 / w_count as f32;  // 每个单元格的宽度（像素）
let cell_h = texture.height as f32 / h_count as f32;

// 第 i 个单元格的 UV 坐标（像素单位！）
let cx = (i % w_count) as f32 * cell_w;   // 左上角 X
let cy = (i / w_count) as f32 * cell_h;   // 左上角 Y
```

每个 Tile 的 `Sprite2D` 使用这些计算出的像素坐标 UV 来引用图集中的特定区域。

#### 7.2.3 音频初始化与播放

```rust
fn init_audio(&mut self) -> Result<AudioManager> {
    // 使用宏在编译时嵌入 .wav 文件
    macro_rules! insert_snd {
        ($name:expr, $dir:expr) => {
            self.sounds.insert(
                $name.to_string(),
                StaticSoundData::from_cursor(Cursor::new(include_bytes!($dir)))?,
            );
        };
    }
    insert_snd!("snd_ominous_cancel", "../snd_ominous_cancel.wav");
    insert_snd!("snd_ominous", "../snd_ominous.wav");

    AudioManager::<DefaultBackend>::new(Default::default())?
}
```

播放时：每次需要播放音效时，clone `StaticSoundData` 并调用 `play`：
```rust
fn handle_sound_effects(&mut self, driver: &EventDriver) {
    if key_state!(driver, KeyCode::KeyX).is_down_true_edge() {
        let snd = self.sounds.get("snd_ominous_cancel").unwrap();
        audio_mgr.play(snd.clone().volume(0.0)).unwrap();
    }
    if driver.mouse().get_mouse_button_state(MouseButton::Left).is_down_edge() {
        let snd = self.sounds.get("snd_ominous").unwrap();
        audio_mgr.play(snd.clone().volume(0.0)).unwrap();
    }
}
```

> **注意**：`StaticSoundData` 是不可变的描述符，`clone` 是轻量操作。每次 `play()` 创建一个新的声音实例。

#### 7.2.4 碰撞与鼠标交互

每个 Tile 都有一个 `Collider::Rect`。使用 `ColliderInstance::contains_point` 检测鼠标是否在瓷砖内：

```rust
fn update_tiles(&mut self, dt: f64, driver: &EventDriver) {
    let world_mouse = camera.screen_to_world(mouse_screen);  // ⚠️ 记得转换坐标！
    let lmb_pressed = driver.mouse().get_mouse_button_state(MouseButton::Left).is_pressed();

    self.hovered_tile = None;
    for (idx, tile) in self.tiles.iter_mut().enumerate().rev() {
        tile.pos += tile.vel * dt as f32;
        tile.rot += tile.rot_vel * dt as f32;

        // 鼠标左键：将瓷砖吸向鼠标
        if lmb_pressed {
            let d = world_mouse - tile.pos;
            tile.vel += d / d.length() * d.length().sqrt();  // 平方根衰减吸引力
        }

        // 碰撞体点包含检测
        let inst = ColliderInstance {
            shape: &tile.collider,
            xform: Transform2D { pos: tile.pos, scale: Vec2::splat(tile.scale), rot: tile.rot },
        };
        if inst.contains_point(world_mouse) && self.hovered_tile.is_none() {
            self.hovered_tile = Some(idx);  // 被悬停的瓷砖索引
        }
    }
}
```

绘制碰撞轮廓时，根据是否被悬停使用不同颜色：
```rust
fn render_colliders(&mut self, driver: &EventDriver) -> Result<()> {
    for (idx, tile) in self.tiles.iter().enumerate() {
        let inst = ColliderInstance { ... };
        let color = if Some(idx) == self.hovered_tile {
            [1.0, 0.8, 0.0, 0.8]  // 悬停 → 金色
        } else {
            [0.0, 1.0, 0.0, 0.3]  // 未悬停 → 绿色半透明
        };
        draw_collider_outline(sb, &inst, color);
    }
}
```

#### 7.2.5 HUD 文字渲染（AtlasText）

HUD 使用**屏幕空间**矩阵，而非世界空间矩阵：

```rust
fn render_hud(&mut self, dt: f64, driver: &EventDriver) -> Result<()> {
    let (w, h) = driver.window_size();

    // ⚠️ 屏幕空间正交矩阵：左上 (0,0)，右下 (w,h)
    //    不要用 camera.vp_matrix()！
    let hud_mvp = glam::Mat4::orthographic_lh(0.0, w, h, 0.0, 0.0, 1.0);
    let hud_vp = hud_mvp.transpose();

    // 排版文字
    let text = format!("FPS: {:.2} | ...\nHello, Rust! 🦀\n...");
    let layout = ctx.atlas_text.layout_text(&text, Metrics::new(24.0, 32.0), ...)?;

    // 阴影（偏移 + 半透明）
    ctx.atlas_text.render_layout_simple(&layout, Vec2::new(10.0, 6.0), [0.0, 0.0, 0.0, 0.75], 0.0, &mut buf);

    // 本体
    ctx.atlas_text.render_layout_simple(&layout, Vec2::new(8.0, 4.0), [1.0, 1.0, 1.0, 1.0], 0.0, &mut buf);

    // ⚠️ 顺序：先 upload，再 push_buffered
    ctx.atlas_text.upload(gfx)?;
    batch.push_buffered(gfx, &hud_vp, &mut buf, |xform| (xform.pos, xform.scale, xform.rot));
}
```

> 💡 **关键区别**：
> - 普通精灵使用 `camera.vp_matrix()`（世界空间）
> - HUD 文字使用 `Mat4::orthographic_lh(0.0, w, h, 0.0, ...)`（屏幕空间）
> - 屏幕空间矩阵让 HUD 固定在窗口位置，不跟随相机移动

#### 7.2.6 push_buffered 的 Shadow 实现

`app_sethsweeper` 展示了两种使用 `push_buffered` 的方式：

**简单方式**（瓷砖）：每个瓷砖推入主精灵和阴影两个 `Sprite2DObject`，使用相同的 pipeline（纹理），不同的位置和颜色。

**高级方式**（Logo）：先 clone 原始对象，修改 transform 添加偏移，设置阴影颜色，再推入缓冲区：

```rust
fn render_demo_sprites(&mut self) -> Result<()> {
    let mut push_sprite = |obj: &Sprite2DObject<...>| {
        // 创建阴影版本
        let mut shadow = obj.clone();
        shadow.transform = shadow.transform.move_by(shadow_offset);  // 偏移
        shadow.color = shadow_color;                                  // 半透明黑色
        buf.push(&shadow);
        buf.push(obj);  // 本体
    };

    let base = Sprite2DObject {
        spr: Sprite2D { origin_px: tex.size_vec2f() * 0.5, ... },
        transform: Transform2D::default(),
        pipeline: TextureInfoArced(tex.clone()),
        color: [1.0; 4],
        layer: 0.0,
    };
    push_sprite(&base);
}
```

#### 7.2.7 渲染顺序和 Pipeline 管理

`push_buffered` 自动按 `layer` 排序并管理 pipeline 切换。在 `render_frame` 中按以下顺序绘制：

1. **背景精灵**（`render_demo_sprites`）：大 Logo + 四个方向的复制
2. **网格**（`render_grid`）：使用 `ShapeBatch2D`
3. **瓷砖**（`render_tiles`）：使用 `SpriteBatch2D`，推入 shadow + sprite
4. **碰撞体轮廓**（`render_colliders`）：使用 `ShapeBatch2D`
5. **HUD**（`render_hud`）：使用 `AtlasText` + `SpriteBatch2D`（屏幕空间）

注意每层之间 `clear_batch()` 和 `submit_and_draw()` 的配合。

---

### 7.3 `app_fish` —— 完整的双人游戏

**路径**：`app_fish/src/app.rs`  
**学习目标**：
- 完全基于 `AtlasText` 的精灵渲染（使用 Emoji 字符代替纹理）
- 双人输入（P1: WASD, P2: 方向键）
- 实体管理（鱼类生成、成长、死亡）
- 粒子系统（吃鱼特效）
- 游戏状态管理（生命值、无敌、减速、游戏结束）
- 复杂 HUD（生命爱心、分数、背景矩形）
- 文字预加载（避免运行时卡顿）
- 长按重置机制
- 相机震动效果
- 音效集成

> ⚠️ **AI 注意**：本示例是引擎的最复杂用例。特别注意：
> - Emoji 作为精灵是通过 `AtlasText` 渲染的，不是普通纹理
> - 文字预加载在初始化时完成，避免运行时卡顿
> - 游戏状态管理中的 timer 都是 `f32`（秒），需要在帧循环中手动递减
> - 粒子系统和相机震动都使用随机数生成（`fastrand`）

#### 7.3.1 项目结构

```
app_fish/
├── Cargo.toml
└── src/
    ├── main.rs          # 入口，窗口创建
    ├── app.rs           # 主应用代码（App, AppContext, 帧循环, HUD, 粒子）
    └── app/
        ├── fish.rs      # Fish / FishSpecies 定义
        ├── fishes.rs    # Fishes 集合管理
        └── grid_render.rs  # 网格绘制工具
```

> 💡 **设计考虑**：将鱼相关逻辑拆分到 `fish.rs` 和 `fishes.rs`，使 `app.rs` 保持可维护性。这是 Rust 项目的良好组织实践。

#### 7.3.2 使用 Emoji 作为精灵

`app_fish` 中所有鱼都使用 Emoji 字符作为纹理，通过 `AtlasText` 渲染：

```rust
// fish.rs
impl FishSpecies {
    pub fn emoji(&self) -> &'static str {
        match self {
            Self::Normal   => "🐟",
            Self::Tropical => "🐠",
            Self::Shark    => "🦈",
            // ...
        }
    }
}
```

创建鱼时，将 Emoji 提交到 AtlasText 进行排版，得到一个可复用的 `TextLayout`：
```rust
let shape_layout = atlas_text.layout_text(
    "🐟",
    Metrics::new(max_size, max_size),  // Emoji 渲染尺寸
    Attrs::new(),
    &gfx.device,
)?;
```

渲染时，通过 `atlas_text.render_layout` 将字形提交到精灵缓冲区：
```rust
pub fn add_to_buffer(&self, _gfx: ..., atlas_text: &mut AtlasText, sprite_buffer: &mut Sprite2DBuffer<...>) {
    // 阴影（偏移 5px）
    atlas_text.render_layout(
        &self.shape_layout, Vec2::ZERO, self.origin,
        self.get_transform().move_by(Vec2::new(5.0, 5.0)),
        [0.0, 0.0, 0.0, 0.3 * self.alpha], 0.0, sprite_buffer,
    );
    // 本体
    atlas_text.render_layout(
        &self.shape_layout, Vec2::ZERO, self.origin,
        self.get_transform(), final_color, 0.0, sprite_buffer,
    );
}
```

> 💡 **优点**：无需加载和维护纹理文件，直接使用系统 Emoji 字体，特别适合原型开发。

#### 7.3.3 文字预加载

为了避免游戏运行时动态光栅化 Emoji 导致的卡顿，`app_fish` 在初始化时预加载所有可能用到的字形：

```rust
fn preload_glyphs(atlas_text: &mut AtlasText, gfx: &D3D11) -> Result<()> {
    // 预加载所有鱼种的 Emoji（用各自的 max_size）
    for &species in ALL_SPECIES {
        let (_, max_s) = species.size_range();
        let emoji = species.emoji();
        let _ = atlas_text.layout_text(emoji, Metrics::new(max_s * 2.0, max_s * 2.0), ...)?;
    }

    // 预加载玩家鱼 Emoji（最大尺寸 512）
    let _ = atlas_text.layout_text("🐠", Metrics::new(512.0, 512.0), ...)?;

    // 预加载 HUD 常用字符
    let _ = atlas_text.layout_text("❤️💀P120分按R或Enter重新开始...", Metrics::new(48.0, 48.0), ...)?;
    Ok(())
}
```

#### 7.3.4 双人输入

P1 使用 WASD，P2 使用方向键：

```rust
fn fish_proc_move(player1: &mut Fish, player2: &mut Fish, driver: &EventDriver, dt: f32) {
    let v = 200.0;
    // P2：方向键
    if driver.keyboard().get_key_state(KeyCode::ArrowRight).is_pressed() {
        player2.pos.x += v * dt; player2.facing = FishFacing::Right;
    }
    if driver.keyboard().get_key_state(KeyCode::ArrowLeft).is_pressed() {
        player2.pos.x -= v * dt; player2.facing = FishFacing::Left;
    }
    // P1：WASD
    if driver.keyboard().get_key_state(KeyCode::KeyD).is_pressed() {
        player1.pos.x += v * dt; player1.facing = FishFacing::Right;
    }
    if driver.keyboard().get_key_state(KeyCode::KeyA).is_pressed() {
        player1.pos.x -= v * dt; player1.facing = FishFacing::Left;
    }
    // ...
}
```

#### 7.3.5 鱼类生成系统

鱼群根据游戏进度（玩家尺寸）自动生成不同种类的鱼：

```rust
fn try_spawn(&mut self, dt: f32, ...) {
    for &species in ALL_SPECIES {
        let unlock = species.unlock_size();
        if self.progress_size < unlock { continue; }  // 未解锁

        // 从 unlock_size 到 unlock_size+50，线性提升生成速率
        let progress = ((self.progress_size - unlock) / 50.0).min(1.0);
        let rate = progress * species.max_spawn_rate();

        // 使用累加器控制生成频率（基于 delta time）
        let acc = self.spawn_acc.entry(species).or_insert(0.0);
        *acc += rate * dt;
        while *acc >= 1.0 && self.fish_list.len() + needs_spawn.len() < 35 {
            *acc -= 1.0;
            spawn_count += 1;
        }
    }
}
```

> 💡 **技巧**：使用 `HashMap<FishSpecies, f32>` 累加器来平滑控制生成速率。每次 update 累加 `rate * dt`，当累加值 ≥ 1 时生成一条鱼并减 1。这种方式避免了浮点误差导致的生成不均匀。

#### 7.3.6 鱼的运动模式

每条鱼都有一个 `MovementPattern` 枚举，定义了 5 种运动模式：

```rust
pub enum MovementPattern {
    HorizontalEntry { from_left: bool, speed: f32 },  // 左右边缘进入
    VerticalEntry { from_top: bool, speed: f32 },      // 上下边缘进入
    Wave { speed, amplitude, frequency, phase, direction },  // 波浪运动
    Linear { velocity: Vec2 },                          // 直线运动
    Stationary,                                         // 静止
}
```

#### 7.3.7 交互检测（吃与被吃）

```rust
pub fn check_interact(&mut self, player_pos: Vec2, player_size: f32) -> EatResult {
    let player_radius = player_size * 0.6;
    let mut to_remove = Vec::new();

    for (i, fish) in self.fish_list.iter().enumerate() {
        if fish.eaten || fish.spawn_fade > 0.0 { continue; }  // 淡入中的鱼不可交互

        let d = fish.pos - player_pos;
        let r_sum = fish.size * 0.6 + player_radius;

        if d.length_squared() > r_sum * r_sum { continue; }  // 未碰撞

        if fish.size < player_size {
            to_remove.push(i);  // 鱼比玩家小 → 被吃掉
        } else {
            hit_by_big = true;  // 鱼比玩家大 → 玩家受伤
        }
    }
    // 从后往前删除，避免索引错乱
    for i in to_remove.into_iter().rev() {
        self.fish_list.swap_remove(i);
    }
}
```

> 💡 **注意**：使用 `swap_remove`（交换删除，O(1)）替代 `remove`（移位删除，O(n)）。因为维护鱼的顺序不重要，这样可以提高性能。

#### 7.3.8 游戏状态管理

**受伤流程**：
1. 检测到玩家与比它大的鱼碰撞
2. 生命值减 1
3. 无敌计时开始（1.5 秒）：期间闪烁免疫
4. 减速计时开始（1.0 秒）：移动速度降至 80%
5. 相机震动累积：每次受伤增加 0.5 秒震动
6. 播放受伤音效

**无敌闪烁**：
```rust
pub fn set_invincible_flash(&mut self, invincible: f32) {
    if invincible > 0.0 {
        let blink = (invincible * 10.0) as i32 % 2 == 0;  // 每 0.1 秒闪烁一次
        self.alpha = if blink { 1.0 } else { 0.2 };
    }
}
```

**相机震动**：
```rust
if ctx.shake_timer > 0.0 {
    ctx.shake_timer -= dt_f;
    let intensity = ctx.shake_intensity * (ctx.shake_timer / 0.3).max(0.0);
    let angle = fastrand::f32() * 6.28;
    ctx.camera.position = Vec2::new(angle.cos() * intensity, angle.sin() * intensity);
} else {
    ctx.camera.position = Vec2::ZERO;
}
```

#### 7.3.9 粒子系统

```rust
pub struct Particle {
    pos: Vec2,
    vel: Vec2,
    lifetime: f32,
    max_lifetime: f32,
    size: f32,
    start_size: f32,
    color: [f32; 4],
}
```

**生成**：当玩家吃鱼时，根据被吃鱼的大小和种类生成粒子：
```rust
fn spawn_eat_particles(&mut self, pos: Vec2, species: FishSpecies, fish_size: f32) {
    let colors = species.bitten_colors();  // 每种鱼有自己的颜色调色板
    let count = (15.0 + size_scale * 10.0).min(60);
    for _ in 0..count {
        // 随机选择颜色、方向、速度、大小
        self.particles.push(Particle {
            pos,
            vel: Vec2::new(angle.cos() * speed, angle.sin() * speed),
            lifetime: 0.4 + fastrand::f32() * 0.8,
            size: sz, start_size: sz,
            color: [c[0], c[1], c[2], 1.0],
        });
    }
}
```

**更新**：每帧衰减摩擦力，缩小子粒尺寸，移除已死亡的粒子。  
**渲染**：使用 `ShapeBatch2D::add_circle_no_uv` 绘制圆形粒子。

#### 7.3.10 HUD 渲染

HUD 使用 `AtlasText` 和 `ShapeBatch2D` 配合实现：

```rust
fn render_hud(ctx: &mut AppContext) -> Result<()> {
    // 1. 创建文字布局（生命值 + 分数）
    let p1_full = format!("P1 {}", p1_display);
    let p1_layout = ctx.atlas_text.layout_text(&p1_full, metrics, attrs, &ctx.gfx.device)?;

    // 2. 绘制背景矩形
    ctx.shape_batch.add_rect_no_uv(
        p1_pos + Vec2::new(-bg_padding, -bg_padding),
        Vec2::new(bg_w1, bg_h), Vec2::ZERO, 0.0,
        [0.0, 0.0, 0.0, 0.6],
    );

    // 3. 渲染文字
    ctx.atlas_text.render_layout(
        &p1_layout, p1_pos, Vec2::ZERO,  // origin = (0,0) 左上对齐
        Transform2D::IDENTITY, text_color, 1.0, &mut ctx.sprite_buf,
    );

    // P2 使用 content_size 实现右对齐
    ctx.atlas_text.render_layout(
        &p2_layout, p2_pos,
        Vec2::new(p2_layout.content_size.x, 0.0),  // origin = (width, 0) 右上对齐
        Transform2D::IDENTITY, text_color, 1.0, &mut ctx.sprite_buf,
    );
}
```

> 💡 **对齐技巧**：`render_layout` 的 `origin` 参数控制文字的锚点。`Vec2::ZERO` = 左上对齐，`Vec2::new(content_size.x, 0.0)` = 右上对齐，`content_size * 0.5` = 居中。

#### 7.3.11 长按重置机制

重置需要长按 R 键 5 秒，防止误触：

```rust
let ks_r = ctx.driver.keyboard().get_key_state(KeyCode::KeyR);
if !ctx.game_over && ks_r.is_pressed() {
    ctx.reset_hold_timer += dt_f;
    if ctx.reset_hold_timer >= ctx.reset_hold_duration {
        ctx.restart();
    }
} else {
    ctx.reset_hold_timer = 0.0;  // 松手即取消
}

// 显示进度条
let progress = (ctx.reset_hold_timer / ctx.reset_hold_duration).min(1.0);
ctx.shape_batch.add_rect_no_uv(bar_pos, Vec2::new(bar_w, bar_h), ...);      // 背景
ctx.shape_batch.add_rect_no_uv(bar_pos + offset, Vec2::new((bar_w - 4.0) * progress, bar_h - 4.0), ...);  // 填充
```

#### 7.3.12 帧循环完整流程

```rust
loop {
    let events = ctx.driver.poll_frame();
    if events.close_requested || events.disconnected { break; }

    let dt = ctx.timer.pre_frame_and_get_delta_time();

    // 开场音效
    if !ctx.intro_played { play_sound(ctx, "snd_ominous"); ctx.intro_played = true; }

    // 长按重置检测
    // 游戏结束时按 R/Enter 直接重置
    // 窗口 resize
    // 更新逻辑
    process_event(ctx, dt)?;
    // 渲染
    render_frame(ctx)?;

    ctx.driver.end_frame();
    ctx.timer.post_frame_fpsc(dt);
    ctx.time_elapsed += dt;
}
```

---

## 8. 附录：已知问题与未来规划

### 8.1 已知问题（⚠️ Warning 汇总）

| 问题 | 模块 | 影响 |
|------|------|------|
| 仅支持 Windows x64 | 全局 | 无法在非 Windows 平台运行 |
| `create_texture_2d` 类型不安全 | d3d11_utils | `cpu_access_flags` 使用 `u32` 非强类型枚举 |
| RGB8 图片自动扩展至 RGBA8 | d3d11_utils | 额外的内存和性能开销 |
| 无 MipMap 支持 | d3d11_utils | 缩放时纹理质量下降 |
| `present` 固定 v-sync | D3D11 | 无法关闭垂直同步 |
| Debug 模式需要 D3D Debug Layer | D3D11 | 没有安装 Debug Layer 时会报错 |
| `ShapeBatch2D::submit_and_draw` 每次重建 VB | ShapeBatch2D | 大量三角形时性能下降 |
| AtlasText 图集空间不可回收 | AtlasText | 长时间运行可能耗尽图集页 |
| AtlasText 整页上传 | AtlasText | 小量文本变化也会上传整页 |
| `MouseInput::device_event` 中滚轮事件注释 | MouseInput | 设备事件的滚轮检测不可用 |
| 无资源管理器 | 全局 | 用户需手动维护纹理 HashMap |

### 8.2 未来规划（🚧 TODO 汇总）

- [ ] 跨平台支持（Vulkan / Metal 后端）
- [ ] 可自定义的窗口属性（标题、尺寸、模式、图标）
- [ ] MipMap 生成
- [ ] 从文件路径直接加载纹理
- [ ] 可配置的 v-sync（`SyncInterval` 参数化）
- [ ] 形状批渲染器性能优化（Map/Discard 代替重建）
- [ ] AtlasText 图集空间回收机制
- [ ] AtlasText 部分上传（只有脏区域）
- [ ] AtlasText 自定义字体加载
- [ ] AtlasText 文本测量和对齐功能
- [ ] 碰撞检测的射线检测（Ray Cast）
- [ ] 宽阶段碰撞剔除（Spatial Hash）
- [ ] 统一的资源管理器/加载器
- [ ] 可选的深度测试（纯 2D 场景不需要）
- [ ] `EngineHandler` 提供更优雅的关闭流程
- [ ] 示例项目中的宏提取到引擎中
- [ ] 内置 ECS 支持（可选）

---

> **作者**：难以置信，这样真的可以跑  
> 如果发现任何 bug 或有改进建议，欢迎提交 Issue 或 PR。