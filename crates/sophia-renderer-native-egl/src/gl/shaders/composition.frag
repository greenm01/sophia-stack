#version 110
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
