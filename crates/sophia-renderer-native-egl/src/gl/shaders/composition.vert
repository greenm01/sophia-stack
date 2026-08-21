#version 110
attribute vec2 position;
attribute vec2 texture_coordinate;
varying vec2 texture_position;
void main() {
    texture_position = texture_coordinate;
    gl_Position = vec4(position, 0.0, 1.0);
}
