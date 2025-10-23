#[cfg(feature = "raylib-renderer")]
pub mod raylib;
#[cfg(feature = "raylib-renderer")]
pub use raylib::clay_raylib_render;

#[cfg(feature = "skia-renderer")]
pub mod skia;
#[cfg(feature = "skia-renderer")]
pub use skia::clay_skia_render;

#[cfg(feature = "macroquad-renderer")]
pub mod macroquad;
#[cfg(feature = "macroquad-renderer")]
pub use macroquad::clay_macroquad_render;
