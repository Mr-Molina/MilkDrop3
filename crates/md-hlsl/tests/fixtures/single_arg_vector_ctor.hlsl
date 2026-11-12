// Task 4.7: single-argument vector constructors splat their scalar.
shader_body
{
    float2 d = float2(rad);
    float3 c = float3(0.5);
    float4 q = float4(ang);
    ret = c * d.x + q.yzw;
}
