struct PSIn {
    float4 pos   : SV_POSITION;
    float2 uv    : TEXCOORD;
    float4 color : COLOR;
};

float4 main(PSIn ps_in) : SV_Target {
    return ps_in.color;
}