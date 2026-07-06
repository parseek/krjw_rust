cbuffer WorldCBuf : register(b0) {
    float4x4 mvp;
};

struct VSIn {
    float2 pos  : POSITION;
    float2 uv   : TEXCOORD;
    float4 color : COLOR;
};

struct PSIn {
    float4 pos   : SV_POSITION;
    float4 color : COLOR;
    float2 uv    : TEXCOORD;
};

PSIn main(VSIn v) {
    PSIn o;
    o.pos = mul(float4(v.pos, 0.0f, 1.0f), mvp);
    o.color = v.color;
    o.uv = v.uv;
    return o;
}