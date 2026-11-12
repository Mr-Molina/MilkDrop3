// Task 4.7: `uv_orig` in a composite shader (alias of `uv`, Task 4.6 define).
shader_body
{
    ret = tex2D(sampler_main, uv_orig).xyz;
}
