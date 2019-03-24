#version 450

layout(location = 0) out vec4 f_color;
layout(location = 0) in vec2 tex_coords;

layout(set = 0, binding = 0) uniform ColorData {
   vec3 color_data; 
} current_color; 

layout(set = 1, binding = 0) uniform sampler2D tex;

void main() {
    f_color = texture(tex, tex_coords);//vec4(current_color.color_data, 1.0); 
}
