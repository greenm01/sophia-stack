#version 110
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
