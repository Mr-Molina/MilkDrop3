// Task 4.7: a `//` comment ending with a backslash is not a line
// continuation: MilkDrop stripped comments before fxc saw the text. \
#define BOOST 2.0 // doubled \
shader_body
{
    ret = tex2D(sampler_main, uv).xyz; // sample the previous frame \
    ret += BOOST * tex2D(sampler_blur1, uv).xyz; // must not be swallowed \
}
