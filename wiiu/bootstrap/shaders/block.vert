#version 330

layout(location = 0) in vec3 aPosition;
layout(location = 1) in vec3 aColor;
layout(location = 2) in vec2 aUv;
layout(location = 3) in float aAlphaCutout;
out vec3 vColor;
out vec2 vUv;
flat out float vAlphaCutout;

void main() {
    gl_Position = vec4(aPosition, 1.0);
    vColor = aColor;
    vUv = aUv;
    vAlphaCutout = aAlphaCutout;
}
