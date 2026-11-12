// Task 4.7: a macro redefined with a different body: fxc warns (X1519,
// macro redefinition) and the new definition applies from there on.
#define GAIN 0.25
#define GAIN 0.75
shader_body
{
    ret = tex2D(sampler_main, uv).xyz * GAIN;
}
