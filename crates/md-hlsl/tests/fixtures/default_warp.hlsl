// Default warp shader (assets/hlsl/warp_ps.fx) hand-wrapped as Task 4.6 /
// plugin.cpp::LoadShaderFromMemory will: prepend assets/hlsl/include.fx,
// then these defines, then the shader_body as `void PS(...)`.
#define rad _rad_ang.x
#define ang _rad_ang.y
#define uv _uv.xy
#define uv_orig _uv.zw
void PS(float4 _vDiffuse : COLOR, float4 _uv : TEXCOORD0, float2 _rad_ang : TEXCOORD1, out float4 _return_value : COLOR0)
{
    float3 ret = 0;

    // sample previous frame
    ret = tex2D( sampler_main, uv ).xyz;

    // darken over time
    ret -= 0.004;

    _return_value = float4(ret.xyz, _vDiffuse.w);
}
