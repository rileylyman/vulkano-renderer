#version 450

layout(location = 0) out vec4 f_color;

layout(set = 0, binding = 0) uniform ColorData {
   vec3 color_data; 
} current_color; 

void main() {
    f_color = vec4(current_color.color_data, 1.0); 
}
