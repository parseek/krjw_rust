#[allow(unused)]
#[cfg(target_os = "windows")]
pub mod d3d11;

#[cfg(target_os = "windows")]
pub use d3d11::D3D11;