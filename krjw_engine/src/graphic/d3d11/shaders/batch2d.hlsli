// Used by Batch2D

cbuffer WorldCBuf : register(b0) {
    float4x4 mvp;
};

Texture2D base_tex : register(t0);
SamplerState base_sampler : register(s0);

struct PSIn_PUC {
    float4 pos   : SV_POSITION;
    float2 uv    : TEXCOORD;
    float4 color : COLOR;
};