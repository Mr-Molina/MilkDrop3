// Task 4.7: identifiers that are WGSL keywords or reserved words.
float loop(float var) { return var * 0.5; }
shader_body
{
    float fn = 0.25;
    float let = loop(fn);
    float3 vec3 = let;
    float override = 1;
    ret = vec3 * override;
}
