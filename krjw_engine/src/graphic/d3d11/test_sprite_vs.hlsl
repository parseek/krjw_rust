struct VSIn {
    float2 pos: POSITION;
    float2 tex: TEXCOORD;
};
struct PSIn {
    float4 pos: SV_POSITION;
    float2 tex: TEXCOORD;
};

cbuffer WorldCBuf: register(b0) {
    float4x4 transform_mvp;
}

cbuffer SpriteCBuf: register(b1) {
    float4x4 transform_spr;
    float4 color;
}


PSIn main(VSIn vs_in) {
    PSIn ps_in;

    ps_in.pos = mul(mul(float4(vs_in.pos, 0.0f, 1.0f), transform_spr), transform_mvp);
    ps_in.tex = vs_in.tex;
    return ps_in;
}