use glow::HasContext as _;

pub(super) unsafe fn compile_shader(
    gl: &glow::Context,
    shader_type: u32,
    source: &str,
) -> Result<glow::Shader, String> {
    let shader = unsafe { gl.create_shader(shader_type)? };
    unsafe {
        gl.shader_source(shader, source);
        gl.compile_shader(shader);
    }
    if unsafe { gl.get_shader_compile_status(shader) } {
        Ok(shader)
    } else {
        let error = unsafe { gl.get_shader_info_log(shader) };
        unsafe { gl.delete_shader(shader) };
        Err(error)
    }
}

pub(super) unsafe fn compile_program(
    gl: &glow::Context,
    fragment_source: &str,
) -> Result<glow::Program, String> {
    let vertex_shader = unsafe { compile_shader(gl, glow::VERTEX_SHADER, VERTEX_SHADER)? };
    let fragment_shader =
        match unsafe { compile_shader(gl, glow::FRAGMENT_SHADER, fragment_source) } {
            Ok(shader) => shader,
            Err(error) => {
                unsafe { gl.delete_shader(vertex_shader) };
                return Err(error);
            }
        };
    let program = match unsafe { gl.create_program() } {
        Ok(program) => program,
        Err(error) => {
            unsafe {
                gl.delete_shader(vertex_shader);
                gl.delete_shader(fragment_shader);
            }
            return Err(error);
        }
    };
    unsafe {
        gl.attach_shader(program, vertex_shader);
        gl.attach_shader(program, fragment_shader);
        gl.bind_attrib_location(program, 0, "position");
        gl.bind_attrib_location(program, 1, "texture_coordinate");
        gl.link_program(program);
        gl.delete_shader(vertex_shader);
        gl.delete_shader(fragment_shader);
    }
    if unsafe { gl.get_program_link_status(program) } {
        Ok(program)
    } else {
        let error = unsafe { gl.get_program_info_log(program) };
        unsafe { gl.delete_program(program) };
        Err(error)
    }
}

pub(super) const VERTEX_SHADER: &str = r#"#version 110
attribute vec2 position;
attribute vec2 texture_coordinate;
varying vec2 texture_position;
void main() {
    texture_position = texture_coordinate;
    gl_Position = vec4(position, 0.0, 1.0);
}
"#;

pub(super) const FRAGMENT_SHADER: &str = r#"#version 110
uniform sampler2D frame;
uniform float opacity;
uniform float source_is_opaque;
varying vec2 texture_position;
void main() {
    vec4 color = texture2D(frame, texture_position);
    if (source_is_opaque > 0.5) {
        color.a = 1.0;
    } else {
        color.rgb = min(color.rgb, vec3(color.a));
    }
    gl_FragColor = vec4(color.rgb * opacity, color.a * opacity);
}
"#;

/// Catmull-Rom reconstruction, weighting light rather than gamma-encoded bytes.
///
/// The kernel keeps a reduced one-pixel stem represented by several coverage
/// values instead of choosing one source row or softening the whole glyph with a
/// two-sample blend. The fixed 4x4 footprint is accepted by the GLES2-class
/// contexts used by the native renderer and keeps work bounded. Being an
/// interpolating kernel it passes through its samples, so it serves enlargements
/// as well as reductions and one program covers both directions.
///
/// The taps are decoded before they are weighted. Averaging the encoded bytes 0
/// and 255 gives 127, which is about a fifth of the light of white rather than
/// half of it, so every resampled edge came out too dark and text read as muddy
/// no matter how good the kernel was. Gamma 2.0 rather than the sRGB curve, for
/// the reason `software/raster_replay.rs` gives where it made the same choice
/// for the CPU path: a squared approximation stays cheap and reproducible, and
/// one transfer function in the tree beats two that could disagree.
///
/// Premultiplied sources must be unpremultiplied across the decode or the alpha
/// is squared along with the colour. Under gamma 2.0 both directions collapse:
/// `(v/a)^2 * a` is `v*v/a`, and `sqrt(L/a) * a` is `sqrt(L*a)`. Alpha itself is
/// never transformed, being coverage rather than light, so it accumulates with
/// the same weights on the raw value.
pub(super) const SHARP_RECONSTRUCTION_FRAGMENT_SHADER: &str = r#"#version 110
uniform sampler2D frame;
uniform float opacity;
uniform float source_is_opaque;
uniform vec2 source_size;
varying vec2 texture_position;

float catmull_rom(float value) {
    float x = abs(value);
    if (x <= 1.0) {
        return ((1.5 * x - 2.5) * x) * x + 1.0;
    }
    if (x < 2.0) {
        return ((-0.5 * x + 2.5) * x - 4.0) * x + 2.0;
    }
    return 0.0;
}

vec4 to_light(vec4 encoded, float opaque) {
    if (opaque > 0.5) {
        return vec4(encoded.rgb * encoded.rgb, encoded.a);
    }
    if (encoded.a <= 0.0) {
        return vec4(0.0);
    }
    return vec4(encoded.rgb * encoded.rgb / encoded.a, encoded.a);
}

vec3 to_bytes(vec3 light, float alpha, float opaque) {
    vec3 safe = max(light, vec3(0.0));
    if (opaque > 0.5) {
        return sqrt(safe);
    }
    if (alpha <= 0.0) {
        return vec3(0.0);
    }
    return sqrt(safe * alpha);
}

void main() {
    vec2 source_position = texture_position * source_size - vec2(0.5);
    vec2 origin = floor(source_position);
    vec2 fraction = source_position - origin;
    vec2 texel = vec2(1.0) / source_size;
    vec4 sum = vec4(0.0);
    float total = 0.0;
    for (int row = -1; row <= 2; row++) {
        float weight_y = catmull_rom(float(row) - fraction.y);
        for (int column = -1; column <= 2; column++) {
            float weight = weight_y * catmull_rom(float(column) - fraction.x);
            vec2 coordinate = (origin + vec2(float(column), float(row)) + vec2(0.5)) * texel;
            sum += to_light(texture2D(frame, coordinate), source_is_opaque) * weight;
            total += weight;
        }
    }
    vec4 mixed = sum / max(total, 0.0001);
    // Clamped before the encode, never after: Catmull-Rom rings below zero on a
    // hard edge and the square root of a negative is not a number, which reaches
    // the screen as a hole rather than as a dark pixel.
    float alpha = clamp(mixed.a, 0.0, 1.0);
    vec4 color = vec4(
        clamp(to_bytes(mixed.rgb, alpha, source_is_opaque), 0.0, 1.0),
        alpha
    );
    if (source_is_opaque > 0.5) {
        color.a = 1.0;
    } else {
        color.rgb = min(color.rgb, vec3(color.a));
    }
    gl_FragColor = vec4(color.rgb * opacity, color.a * opacity);
}
"#;
