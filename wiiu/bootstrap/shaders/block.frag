#version 330

uniform sampler2D uTexture;
in vec3 vColor;
in vec2 vUv;
out vec4 fragColor;

void main() {
    fragColor = texture(uTexture, vUv) * vec4(vColor, 1.0);
}
