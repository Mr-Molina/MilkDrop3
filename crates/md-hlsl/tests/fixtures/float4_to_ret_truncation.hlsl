// Task 4.7: a float4 texture result assigned to the float3 `ret` truncates
// (fxc warning X3206).
shader_body
{
    ret = tex2D(sampler_main, uv);
    ret += 0.5 * tex2D(sampler_blur1, uv);
}
