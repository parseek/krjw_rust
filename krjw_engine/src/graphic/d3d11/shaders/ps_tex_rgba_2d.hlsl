Texture2D g_tex : register(t0);
SamplerState g_sampler : register(s0);

struct PSIn {
    float4 pos   : SV_POSITION;
    float2 uv    : TEXCOORD;
    float4 color : COLOR;
};

float4 main(PSIn ps_in) : SV_Target {
    return g_tex.Sample(g_sampler, ps_in.uv) * ps_in.color;
}