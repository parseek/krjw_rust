struct VSIn {
    float3 pos   : POSITION;
    float2 uv    : TEXCOORD;
    float4 color : COLOR;
};

#include "batch2d.hlsli"

PSIn_PUC main(VSIn v) {
    PSIn_PUC o;
    o.pos = mul(float4(v.pos, 1.0f), mvp);
    o.uv = v.uv;
    o.color = v.color;
    return o;
}