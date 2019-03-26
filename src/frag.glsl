#version 450

layout(location = 0) out vec4 f_color;
//layout(location = 0) in vec2 tex_coords;
layout(location = 0) in vec3 v_normal;

//layout(set = 0, binding = 0) uniform ColorData {
//   vec3 color_data; 
//} current_color; 

//layout(set = 1, binding = 0) uniform sampler2D tex;

const vec3 LIGHT = vec3(0.0,0.0,1.0);

void main() {
    //f_color = texture(tex, tex_coords);
    //vec4(current_color.color_data, 1.0); 
    float brightness = dot(normalize(v_normal), normalize(LIGHT));
    vec3 dark_color = vec3(0.6, 0.0, 0.0);
    vec3 light_color = vec3(1.0, 0.0, 0.0);
    f_color = vec4(mix(dark_color, light_color, brightness), 1.0);
}
