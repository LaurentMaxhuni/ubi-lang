struct VertexOut {
    @builtin(position) position: vec4<f32>,
    @location(0) world: vec2<f32>, @location(1) local: vec2<f32>,
    @location(2) size: vec2<f32>, @location(3) uv: vec2<f32>,
    @location(4) color: vec4<f32>, @location(5) clip: vec4<f32>,
    @location(6) params: vec2<f32>,
};
@vertex fn vs_main(@location(0) position: vec2<f32>, @location(1) world: vec2<f32>,
    @location(2) local: vec2<f32>, @location(3) size: vec2<f32>,
    @location(4) uv: vec2<f32>, @location(5) color: vec4<f32>,
    @location(6) clip: vec4<f32>, @location(7) params: vec2<f32>) -> VertexOut {
    return VertexOut(vec4(position,0.0,1.0),world,local,size,uv,color,clip,params);
}
@group(0) @binding(0) var atlas: texture_2d<f32>;
@group(0) @binding(1) var atlas_sampler: sampler;
fn linear(c:vec3<f32>)->vec3<f32>{return select(pow((c+0.055)/1.055,vec3(2.4)),c/12.92,c<=vec3(0.04045));}
@fragment fn fs_main(v:VertexOut)->@location(0) vec4<f32>{
    let radius=min(v.params.x,min(v.size.x,v.size.y)*0.5);
    let q=abs(v.local-v.size*0.5)-(v.size*0.5-vec2(radius));
    let distance=length(max(q,vec2(0.0)))+min(max(q.x,q.y),0.0)-radius;
    let coverage=1.0-smoothstep(-max(fwidth(distance),0.4),max(fwidth(distance),0.4),distance);
    let glyph=textureSample(atlas,atlas_sampler,v.uv).r;
    let alpha=v.color.a*select(coverage,glyph,v.params.y>0.5);
    if v.world.x<v.clip.x||v.world.y<v.clip.y||v.world.x>=v.clip.z||v.world.y>=v.clip.w{discard;}
    return vec4(linear(v.color.rgb),alpha);
}
