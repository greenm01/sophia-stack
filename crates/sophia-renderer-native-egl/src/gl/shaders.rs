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

// The GLSL lives in its own files rather than in string literals here.
//
// `include_str!` embeds them at compile time, so nothing is read at runtime and
// there is no asset to deploy or fail to find -- the only thing that changes is
// which language the editor, the diff, and any future validator see. A shader
// error is otherwise not discoverable until a GPU refuses it, at which point the
// session takes the degraded path and keeps running.
//
// What each file is *for* is documented here, next to the renderer that binds
// it. What a particular line does is commented in the GLSL, next to that line.

/// Shared by both fragment programs. Passes the UV through and takes clip-space
/// position straight from the attribute; it does nothing with colour.
pub(super) const VERTEX_SHADER: &str = include_str!("shaders/composition.vert");

/// One texture fetch, no reconstruction. Serves exact 1:1 draws, and the
/// fallback when the reconstruction program did not compile.
pub(super) const FRAGMENT_SHADER: &str = include_str!("shaders/composition.frag");

/// Catmull-Rom reconstruction, weighting light rather than gamma-encoded bytes.
///
/// The kernel keeps a reduced one-pixel stem represented by several coverage
/// values instead of choosing one source row or softening the whole glyph with a
/// two-sample blend. The fixed 4x4 footprint is accepted by the GLES2-class
/// contexts used by the native renderer and keeps work bounded. Being an
/// interpolating kernel it passes through its samples, so it serves enlargements
/// as well as reductions and one program covers both directions.
///
/// It requires `NEAREST` texture filtering. The kernel gathers its own footprint
/// at texel centres, so a hardware `LINEAR` filter would blend those texels in
/// gamma-encoded space before this ever ran and undo the correction invisibly.
/// `composition_draw_plan` is what guarantees it, by choosing the filter and the
/// program together.
///
/// Gamma 2.0 rather than the sRGB curve, for the reason
/// `software/raster_replay.rs` gives where it made the same choice for the CPU
/// path: a squared approximation stays cheap across sixteen taps and keeps one
/// transfer function in the tree instead of two that could disagree.
pub(super) const SHARP_RECONSTRUCTION_FRAGMENT_SHADER: &str =
    include_str!("shaders/sharp_reconstruction.frag");
