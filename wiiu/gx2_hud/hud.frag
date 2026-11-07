#version 330
uniform sampler2D uAtlas;
in vec4 vColor;
in vec2 vUv;
out vec4 fragColor;
void main() {
    vec4 c = texture(uAtlas, vUv) * vColor;
    if (c.a < 0.004) discard;
    fragColor = c;
}
