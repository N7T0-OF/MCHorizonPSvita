#version 330
layout(location = 0) in vec4 aPosition;
layout(location = 1) in vec4 aColor;
layout(location = 2) in vec2 aUv;
out vec4 vColor;
out vec2 vUv;
void main() {
    gl_Position = aPosition;
    vColor = aColor;
    vUv = aUv;
}
