use macroquad::prelude::*;
use crate::{math::BoundingBox, render_commands::{CornerRadii, RenderCommand, RenderCommandConfig}};

#[cfg(feature = "macroquad-text-styling")]
use crate::renderers::macroquad_text_styling::{parse_text_lines, render_styled_text, StyledSegment};
#[cfg(feature = "macroquad-text-styling")]
use std::collections::HashMap;

const PIXELS_PER_POINT: f32 = 2.0;

#[cfg(feature = "macroquad-text-styling")]
static ANIMATION_TRACKER: std::sync::LazyLock<std::sync::Mutex<HashMap<String, (usize, f64)>>> = std::sync::LazyLock::new(|| std::sync::Mutex::new(HashMap::new()));

fn clay_to_macroquad_color(clay_color: &crate::color::Color) -> Color {
    Color {
        r: clay_color.r / 255.0,
        g: clay_color.g / 255.0,
        b: clay_color.b / 255.0,
        a: clay_color.a / 255.0,
    }
}

pub fn clay_macroquad_render<'a, CustomElementData: 'a>(
    commands: impl Iterator<Item = RenderCommand<'a, Texture2D, CustomElementData>>,
    fonts: &[Font],
    handle_custom_command: impl Fn(&RenderCommand<'a, Texture2D, CustomElementData>)
) {
    let mut clip = None;
    fn rounded_rectangle_texture(cr: &CornerRadii, bb: &BoundingBox, clip: &Option<(i32, i32, i32, i32)>) -> Texture2D {
        let render_target = render_target(bb.width as u32, bb.height as u32);
        render_target.texture.set_filter(FilterMode::Linear);
        let mut cam = Camera2D::from_display_rect(Rect::new(0.0, 0.0, bb.width, bb.height));
        cam.render_target = Some(render_target.clone());
        set_camera(&cam);
        unsafe {
            get_internal_gl().quad_gl.scissor(None);
        };

        // Edges
        // Top edge
        if cr.top_left > 0.0 || cr.top_right > 0.0 {
            draw_rectangle(
                cr.top_left,
                0.0,
                bb.width - cr.top_left - cr.top_right,
                bb.height - cr.bottom_left.max(cr.bottom_right),
                WHITE
            );
        }
        // Left edge
        if cr.top_left > 0.0 || cr.bottom_left > 0.0 {
            draw_rectangle(
                0.0,
                cr.top_left,
                bb.width - cr.top_right.max(cr.bottom_right),
                bb.height - cr.top_left - cr.bottom_left,
                WHITE
            );
        }
        // Bottom edge
        if cr.bottom_left > 0.0 || cr.bottom_right > 0.0 {
            draw_rectangle(
                cr.bottom_left,
                cr.top_left.max(cr.top_right),
                bb.width - cr.bottom_left - cr.bottom_right,
                bb.height - cr.top_left.max(cr.top_right),
                WHITE
            );
        }
        // Right edge
        if cr.top_right > 0.0 || cr.bottom_right > 0.0 {
            draw_rectangle(
                bb.width - cr.top_right,
                cr.top_right,
                bb.width - cr.top_left.max(cr.bottom_left),
                bb.height - cr.top_right - cr.bottom_right,
                WHITE
            );
        }

        // Corners
        // Top-left corner
        if cr.top_left > 0.0 {
            draw_circle(
                cr.top_left,
                cr.top_left,
                cr.top_left,
                WHITE,
            );
        }
        // Top-right corner
        if cr.top_right > 0.0 {
            draw_circle(
                bb.width - cr.top_right,
                cr.top_right,
                cr.top_right,
                WHITE,
            );
        }
        // Bottom-left corner
        if cr.bottom_left > 0.0 {
            draw_circle(
                cr.bottom_left,
                bb.height - cr.bottom_left,
                cr.bottom_left,
                WHITE,
            );
        }
        // Bottom-right corner
        if cr.bottom_right > 0.0 {
            draw_circle(
                bb.width - cr.bottom_right,
                bb.height - cr.bottom_right,
                cr.bottom_right,
                WHITE,
            );
        }

        set_default_camera();
        unsafe {
            get_internal_gl().quad_gl.scissor(*clip);
        }
        render_target.texture
    }
    fn resize(texture: &Texture2D, height: f32, width: f32, clip: &Option<(i32, i32, i32, i32)>) -> Texture2D {
        let render_target = render_target(width as u32, height as u32);
        render_target.texture.set_filter(FilterMode::Linear);
        let mut cam = Camera2D::from_display_rect(Rect::new(0.0, 0.0, width, height));
        cam.render_target = Some(render_target.clone());
        set_camera(&cam);
        unsafe {
            get_internal_gl().quad_gl.scissor(None);
        };
        draw_texture_ex(
            texture,
            0.0,
            0.0,
            WHITE,
            DrawTextureParams {
                dest_size: Some(Vec2::new(width, height)),
                flip_y: true,
                ..Default::default()
            },
        );
        set_default_camera();
        unsafe {
            get_internal_gl().quad_gl.scissor(*clip);
        }
        render_target.texture
    }

    #[cfg(feature = "macroquad-text-styling")]
    let mut style_stack: Vec<String> = Vec::new();
    #[cfg(feature = "macroquad-text-styling")]
    let mut total_char_index = 0;
    
    for command in commands {
        match &command.config {
            #[cfg(feature = "macroquad-text-styling")]
            RenderCommandConfig::Text(config) => {
                use crate::renderers::macroquad_text_styling::StyledSegment;

                let bb = command.bounding_box;
                let font_size = config.font_size as f32;
                let font = Some(&fonts[config.font_id as usize]);
                let default_color = clay_to_macroquad_color(&config.color);

                let normal_render = || {
                    let x_scale = if config.letter_spacing > 0 {
                        bb.width / measure_text(
                            config.text,
                            font,
                            config.font_size as u16,
                            1.0
                        ).width
                    } else {
                        1.0
                    };
                    draw_text_ex(
                        config.text,
                        bb.x,
                        bb.y + bb.height,
                        TextParams {
                            font_size: config.font_size as u16,
                            font,
                            font_scale: 1.0,
                            font_scale_aspect: x_scale,
                            rotation: 0.0,
                            color: default_color
                        }
                    );
                };
                
                let mut in_style_def = false;
                let mut escaped = false;
                let mut failed = false;
                
                let mut text_buffer = String::new();
                let mut style_buffer = String::new();

                let line = config.text.to_string();
                let mut segments: Vec<StyledSegment> = Vec::new();

                for c in line.chars() {
                    if escaped {
                        if in_style_def {
                            style_buffer.push(c);
                        } else {
                            text_buffer.push(c);
                        }
                        escaped = false;
                        continue;
                    }

                    match c {
                        '\\' => {
                            escaped = true;
                        }
                        '{' => {
                            if in_style_def {
                                style_buffer.push(c); 
                            } else {
                                if !text_buffer.is_empty() {
                                    segments.push(StyledSegment {
                                        text: text_buffer.clone(),
                                        styles: style_stack.clone(),
                                    });
                                    text_buffer.clear();
                                }
                                in_style_def = true;
                            }
                        }
                        '|' => {
                            if in_style_def {
                                style_stack.push(style_buffer.clone());
                                style_buffer.clear();
                                in_style_def = false;
                            } else {
                                text_buffer.push(c);
                            }
                        }
                        '}' => {
                            if in_style_def {
                                style_buffer.push(c);
                            } else {
                                if !text_buffer.is_empty() {
                                    segments.push(StyledSegment {
                                        text: text_buffer.clone(),
                                        styles: style_stack.clone(),
                                    });
                                    text_buffer.clear();
                                }
                                
                                if style_stack.pop().is_none() {
                                    failed = true;
                                    break;
                                }
                            }
                        }
                        _ => {
                            if in_style_def {
                                style_buffer.push(c);
                            } else {
                                text_buffer.push(c);
                            }
                        }
                    }
                }
                if !(failed || in_style_def) {
                    if !text_buffer.is_empty() {
                        segments.push(StyledSegment {
                            text: text_buffer.clone(),
                            styles: style_stack.clone(),
                        });
                    }
                    
                    let time = get_time();
                    
                    let cursor_x = std::cell::Cell::new(bb.x);
                    let cursor_y = bb.y + bb.height;
                    let mut pending_renders = Vec::new();
                    
                    let x_scale = if config.letter_spacing > 0 {
                        bb.width / measure_text(
                            config.text,
                            Some(&fonts[config.font_id as usize]),
                            config.font_size as u16,
                            1.0
                        ).width
                    } else {
                        1.0
                    };
                    {
                        let mut tracker = ANIMATION_TRACKER.lock().unwrap();
                        render_styled_text(
                            &segments,
                            time,
                            font_size,
                            &mut *tracker,
                            &mut total_char_index,
                            |text, tr, style_color| {
                                let text_string = text.to_string();
                                let text_width = measure_text(&text_string, font, config.font_size as u16, 1.0).width;
                                
                                let color = Color::new(style_color.r, style_color.g, style_color.b, style_color.a);
                                let x = cursor_x.get();
                                
                                pending_renders.push((x, text_string, tr, color));
                                
                                cursor_x.set(x + text_width*x_scale);
                            },
                            |text, tr, style_color| {
                                let text_string = text.to_string();
                                let color = Color::new(style_color.r, style_color.g, style_color.b, style_color.a);
                                let x = cursor_x.get();
                                
                                draw_text_ex(
                                    &text_string,
                                    x + tr.x*x_scale,
                                    cursor_y + tr.y,
                                    TextParams {
                                        font_size: config.font_size as u16,
                                        font,
                                        font_scale: tr.scale_y.max(0.01),
                                        font_scale_aspect: if tr.scale_y > 0.01 { tr.scale_x / tr.scale_y * x_scale } else { x_scale },
                                        rotation: tr.rotation.to_radians(),
                                        color
                                    }
                                );
                            }
                        );
                    }
                    for (x, text_string, tr, color) in pending_renders {
                        draw_text_ex(
                            &text_string,
                            x + tr.x*x_scale,
                            cursor_y + tr.y,
                            TextParams {
                                font_size: config.font_size as u16,
                                font,
                                font_scale: tr.scale_y.max(0.01),
                                font_scale_aspect: if tr.scale_y > 0.01 { tr.scale_x / tr.scale_y * x_scale } else { x_scale },
                                rotation: tr.rotation.to_radians(),
                                color
                            }
                        );
                    }
                } else {
                    if in_style_def {
                        warn!("Style definition didn't end! Here is what we tried to render: {}", config.text);
                    } else if failed {
                        warn!("Encountered }} without opened style! Make sure to escape curly braces with \\. Here is what we tried to render: {}", config.text);
                    }
                    normal_render();
                }
            }
            #[cfg(not(feature = "macroquad-text-styling"))]
            RenderCommandConfig::Text(config) => {
                let bb = command.bounding_box;
                let color = clay_to_macroquad_color(&config.color);

                let x_scale = if config.letter_spacing > 0 {
                    bb.width / measure_text(
                        config.text,
                        Some(&fonts[config.font_id as usize]),
                        config.font_size as u16,
                        1.0
                    ).width
                } else {
                    1.0
                };
                draw_text_ex(
                    &config.text,
                    bb.x,
                    bb.y + bb.height,
                    TextParams {
                        font_size: config.font_size as u16,
                        font: Some(&fonts[config.font_id as usize]),
                        font_scale: 1.0,
                        font_scale_aspect: x_scale,
                        rotation: 0.0,
                        color
                    }
                );
            }
            RenderCommandConfig::Image(image) => {
                let bb = command.bounding_box;
                let cr = &image.corner_radii;
                let mut tint = clay_to_macroquad_color(&image.background_color);
                if tint == Color::new(0.0, 0.0, 0.0, 0.0) {
                    tint = Color::new(1.0, 1.0, 1.0, 1.0);
                }
                if cr.top_left == 0.0 && cr.top_right == 0.0 && cr.bottom_left == 0.0 && cr.bottom_right == 0.0 {
                    draw_texture_ex(
                        image.data,
                        bb.x,
                        bb.y,
                        tint,
                        DrawTextureParams {
                            dest_size: Some(Vec2::new(bb.width, bb.height)),
                            ..Default::default()
                        },
                    );
                } else {
                    let mut resized_image: Image = resize(&image.data, bb.height, bb.width, &clip).get_texture_data();
                    let rounded_rect: Image = rounded_rectangle_texture(cr, &bb, &clip).get_texture_data();

                    for i in 0..resized_image.bytes.len()/4 {
                        let this_alpha = resized_image.bytes[i * 4 + 3] as f32 / 255.0;
                        let mask_alpha = rounded_rect.bytes[i * 4 + 3] as f32 / 255.0;
                        resized_image.bytes[i * 4 + 3] = (this_alpha * mask_alpha * 255.0) as u8;
                    }
                    
                    draw_texture_ex(
                        &Texture2D::from_image(&resized_image),
                        bb.x,
                        bb.y,
                        tint,
                        DrawTextureParams {
                            dest_size: Some(Vec2::new(bb.width, bb.height)),
                            ..Default::default()
                        },
                    );
                }
            }
            RenderCommandConfig::Rectangle(config) => {
                let bb = command.bounding_box;
                let color = clay_to_macroquad_color(&config.color);
                let cr = &config.corner_radii;

                if cr.top_left == 0.0 && cr.top_right == 0.0 && cr.bottom_left == 0.0 && cr.bottom_right == 0.0 {
                    draw_rectangle(
                        bb.x,
                        bb.y,
                        bb.width,
                        bb.height,
                        color
                    );
                } else if color.a == 1.0 {
                    // Edges
                    // Top edge
                    if cr.top_left > 0.0 || cr.top_right > 0.0 {
                        draw_rectangle(
                            bb.x + cr.top_left,
                            bb.y,
                            bb.width - cr.top_left - cr.top_right,
                            bb.height - cr.bottom_left.max(cr.bottom_right),
                            color
                        );
                    }
                    // Left edge
                    if cr.top_left > 0.0 || cr.bottom_left > 0.0 {
                        draw_rectangle(
                            bb.x,
                            bb.y + cr.top_left,
                            bb.width - cr.top_right.max(cr.bottom_right),
                            bb.height - cr.top_left - cr.bottom_left,
                            color
                        );
                    }
                    // Bottom edge
                    if cr.bottom_left > 0.0 || cr.bottom_right > 0.0 {
                        draw_rectangle(
                            bb.x + cr.bottom_left,
                            bb.y + cr.top_left.max(cr.top_right),
                            bb.width - cr.bottom_left - cr.bottom_right,
                            bb.height - cr.top_left.max(cr.top_right),
                            color
                        );
                    }
                    // Right edge
                    if cr.top_right > 0.0 || cr.bottom_right > 0.0 {
                        draw_rectangle(
                            bb.x + cr.top_left.max(cr.bottom_left),
                            bb.y + cr.top_right,
                            bb.width - cr.top_left.max(cr.bottom_left),
                            bb.height - cr.top_right - cr.bottom_right,
                            color
                        );
                    }

                    // Corners
                    // Top-left corner
                    if cr.top_left > 0.0 {
                        draw_circle(
                            bb.x + cr.top_left,
                            bb.y + cr.top_left,
                            cr.top_left,
                            color,
                        );
                    }
                    // Top-right corner
                    if cr.top_right > 0.0 {
                        draw_circle(
                            bb.x + bb.width - cr.top_right,
                            bb.y + cr.top_right,
                            cr.top_right,
                            color,
                        );
                    }
                    // Bottom-left corner
                    if cr.bottom_left > 0.0 {
                        draw_circle(
                            bb.x + cr.bottom_left,
                            bb.y + bb.height - cr.bottom_left,
                            cr.bottom_left,
                            color,
                        );
                    }
                    // Bottom-right corner
                    if cr.bottom_right > 0.0 {
                        draw_circle(
                            bb.x + bb.width - cr.bottom_right,
                            bb.y + bb.height - cr.bottom_right,
                            cr.bottom_right,
                            color,
                        );
                    }
                } else {
                    draw_texture_ex(
                        &rounded_rectangle_texture(cr, &bb, &clip),
                        bb.x,
                        bb.y,
                        color,
                        DrawTextureParams {
                            dest_size: Some(Vec2::new(bb.width, bb.height)),
                            flip_y: true,
                            ..Default::default()
                        },
                    );
                }
            }
            RenderCommandConfig::Border(config) => {
                let bb = command.bounding_box;
                let bw = &config.width;
                let cr = &config.corner_radii;
                let color = clay_to_macroquad_color(&config.color);
                if cr.top_left == 0.0 && cr.top_right == 0.0 && cr.bottom_left == 0.0 && cr.bottom_right == 0.0 {
                    if bw.left == bw.right && bw.left == bw.top && bw.left == bw.bottom {
                        let border_width = bw.left as f32;
                        draw_rectangle_lines(
                            bb.x - border_width / 2.0,
                            bb.y - border_width / 2.0,
                            bb.width + border_width,
                            bb.height + border_width,
                            border_width,
                            color
                        );
                    } else {
                        // Top edge
                        draw_line(
                            bb.x,
                            bb.y - bw.top as f32 / 2.0,
                            bb.x + bb.width,
                            bb.y - bw.top as f32 / 2.0,
                            bw.top as f32,
                            color
                        );
                        // Left edge
                        draw_line(
                            bb.x - bw.left as f32 / 2.0,
                            bb.y,
                            bb.x - bw.left as f32 / 2.0,
                            bb.y + bb.height,
                            bw.left as f32,
                            color
                        );
                        // Bottom edge
                        draw_line(
                            bb.x,
                            bb.y + bb.height + bw.bottom as f32 / 2.0,
                            bb.x + bb.width,
                            bb.y + bb.height + bw.bottom as f32 / 2.0,
                            bw.bottom as f32,
                            color
                        );
                        // Right edge
                        draw_line(
                            bb.x + bb.width + bw.right as f32 / 2.0,
                            bb.y,
                            bb.x + bb.width + bw.right as f32 / 2.0,
                            bb.y + bb.height,
                            bw.right as f32,
                            color
                        );
                    }
                } else {
                    // Edges
                    // Top edge
                    draw_line(
                        bb.x + cr.top_left,
                        bb.y - bw.top as f32 / 2.0,
                        bb.x + bb.width - cr.top_right,
                        bb.y - bw.top as f32 / 2.0,
                        bw.top as f32,
                        color
                    );
                    // Left edge
                    draw_line(
                        bb.x - bw.left as f32 / 2.0,
                        bb.y + cr.top_left,
                        bb.x - bw.left as f32 / 2.0,
                        bb.y + bb.height - cr.bottom_left,
                        bw.left as f32,
                        color
                    );
                    // Bottom edge
                    draw_line(
                        bb.x + cr.bottom_left,
                        bb.y + bb.height + bw.bottom as f32 / 2.0,
                        bb.x + bb.width - cr.bottom_right,
                        bb.y + bb.height + bw.bottom as f32 / 2.0,
                        bw.bottom as f32,
                        color
                    );
                    // Right edge
                    draw_line(
                        bb.x + bb.width + bw.right as f32 / 2.0,
                        bb.y + cr.top_right,
                        bb.x + bb.width + bw.right as f32 / 2.0,
                        bb.y + bb.height - cr.bottom_right,
                        bw.right as f32,
                        color
                    );

                    // Corners
                    // Top-left corner
                    if cr.top_left > 0.0 {
                        let width = bw.left.max(bw.top) as f32;
                        let points = ((std::f32::consts::PI * (cr.top_left + width)) / 2.0 / PIXELS_PER_POINT).max(5.0);
                        draw_arc(
                            bb.x + cr.top_left,
                            bb.y + cr.top_left,
                            points as u8,
                            cr.top_left,
                            180.0,
                            bw.left as f32,
                            90.0,
                            color
                        );
                    }
                    // Top-right corner
                    if cr.top_right > 0.0 {
                        let width = bw.top.max(bw.right) as f32;
                        let points = ((std::f32::consts::PI * (cr.top_right + width)) / 2.0 / PIXELS_PER_POINT).max(5.0);
                        draw_arc(
                            bb.x + bb.width - cr.top_right,
                            bb.y + cr.top_right,
                            points as u8,
                            cr.top_right,
                            270.0,
                            bw.top as f32,
                            90.0,
                            color
                        );
                    }
                    // Bottom-left corner
                    if cr.bottom_left > 0.0 {
                        let width = bw.left.max(bw.bottom) as f32;
                        let points = ((std::f32::consts::PI * (cr.bottom_left + width)) / 2.0 / PIXELS_PER_POINT).max(5.0);
                        draw_arc(
                            bb.x + cr.bottom_left,
                            bb.y + bb.height - cr.bottom_left,
                            points as u8,
                            cr.bottom_left,
                            90.0,
                            bw.bottom as f32,
                            90.0,
                            color
                        );
                    }
                    // Bottom-right corner
                    if cr.bottom_right > 0.0 {
                        let width = bw.bottom.max(bw.right) as f32;
                        let points = ((std::f32::consts::PI * (cr.bottom_right + width)) / 2.0 / PIXELS_PER_POINT).max(5.0);
                        draw_arc(
                            bb.x + bb.width - cr.bottom_right,
                            bb.y + bb.height - cr.bottom_right,
                            points as u8,
                            cr.bottom_right,
                            0.0,
                            bw.right as f32,
                            90.0,
                            color
                        );
                    }
                }
            }
            RenderCommandConfig::ScissorStart() => {
                let bb = command.bounding_box;
                clip = Some((
                    bb.x as i32,
                    bb.y as i32,
                    bb.width as i32,
                    bb.height as i32,
                ));
                unsafe {
                    get_internal_gl().quad_gl.scissor(clip);
                }
            }
            RenderCommandConfig::ScissorEnd() => {
                clip = None;
                unsafe {
                    get_internal_gl().quad_gl.scissor(None);
                }
            }
            RenderCommandConfig::Custom(_) => {
                handle_custom_command(&command);
            }
            RenderCommandConfig::None() => {}
        }
    }
}

pub fn create_measure_text_function(
    fonts: Vec<Font>,
) -> impl Fn(&str, &crate::TextConfig) -> crate::Dimensions + 'static {
    move |text: &str, config: &crate::TextConfig| {
        #[cfg(feature = "macroquad-text-styling")]
        let cleaned_text = {
            // Remove macroquad_text_styling tags, handling escapes
            let mut result = String::new();
            let mut in_style_def = false;
            let mut escaped = false;
            for c in text.chars() {
                if escaped {
                    result.push(c);
                    escaped = false;
                    continue;
                }
                match c {
                    '\\' => {
                        escaped = true;
                    }
                    '{' => {
                        in_style_def = true;
                    }
                    '|' => {
                        if in_style_def {
                            in_style_def = false;
                        } else {
                            result.push(c);
                        }
                    }
                    '}' => {
                        // Nothing
                    }
                    _ => {
                        if !in_style_def {
                            result.push(c);
                        }
                    }
                }
            }
            if in_style_def {
                panic!("Ended inside a style definition while cleaning text for measurement! Make sure to escape curly braces with \\. Here is what we tried to measure: {}", text);
            }
            result
        };
        #[cfg(not(feature = "macroquad-text-styling"))]
        let cleaned_text = text.to_string();
        let measured = macroquad::text::measure_text(
            &cleaned_text,
            Some(&fonts[config.font_id as usize]),
            config.font_size,
            1.0,
        );
        let added_space = (text.chars().count().max(1) - 1) as f32 * config.letter_spacing as f32;
        crate::Dimensions::new(measured.width + added_space, measured.height)
    }
}