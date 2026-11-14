// Task 4.7: `tex2D(...).xyz` into the float3 `ret`, and as a bare
// expression statement.
shader_body
{
    tex2D(sampler_main, uv).xyz;
    ret = tex2D(sampler_main, uv).xyz;
}
