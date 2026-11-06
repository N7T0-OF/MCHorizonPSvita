#version 330

uniform sampler2D uTexture;
in vec3 vColor;
in vec2 vUv;
flat in float vAlphaCutout;
out vec4 fragColor;

void main() {
    vec4 texel = texture(uTexture, vUv);
    if (vAlphaCutout > 0.5 && texel.a < 0.5) {
        discard;
    }
    fragColor = texel * vec4(vColor, 1.0);
}
