// Task 4.7: `;` after the closing `}` of shader_body (the C++ split drops
// everything after the last `}`).
shader_body
{
    ret = tex2D(sampler_main, uv).xyz;
};
