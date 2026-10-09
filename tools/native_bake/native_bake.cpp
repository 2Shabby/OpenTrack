// Offline acceleration of the existing 10 cm sampler/partition/verification.
// This extension is loaded only by the authoring tool, never by the game.
#include <godot_cpp/classes/fast_noise_lite.hpp>
#include <godot_cpp/classes/ref_counted.hpp>
#include <godot_cpp/core/class_db.hpp>
#include <godot_cpp/godot.hpp>
#include <godot_cpp/variant/dictionary.hpp>
#include <godot_cpp/variant/packed_float32_array.hpp>
#include <godot_cpp/variant/packed_int32_array.hpp>
#include <godot_cpp/variant/packed_vector3_array.hpp>
#include <godot_cpp/variant/rect2i.hpp>
#include <godot_cpp/variant/vector4.hpp>
#include <algorithm>
#include <cmath>
#include <cstdint>
#include <unordered_map>
#include <vector>
using namespace godot;
namespace {
constexpr int N=320, STRIDE=321;
constexpr double SPACING=0.1, SURFACE_ERROR=0.1, SMOOTH_ERROR=0.005;
uint64_t key(int x,int z){return uint64_t(uint32_t(x))<<32 | uint32_t(z);}
struct Segment {Vector2 a,b,edge,height;float squared;};
Vector2 closest(Vector2 p,const Segment &s){float t=std::clamp(s.edge.dot(p-s.a)/s.squared,0.0f,1.0f);return s.a+s.edge*t;}
struct Chunk {
 PackedFloat32Array samples,distances;
 PackedInt32Array owners;
 Array leaves;
 Vector2i coord;
 std::vector<int> smooth_prefix;
 double smooth=0,maximum_error=0,maximum_gradient=0;
 void prefix(){
  smooth_prefix.assign(322*322,0);
  auto d=distances.ptr();
  for(int z=0;z<=N;z++)for(int x=0;x<=N;x++){
   int p=(z+1)*322+x+1;
   smooth_prefix[p]=(d[z*STRIDE+x]<=smooth)+smooth_prefix[p-1]+smooth_prefix[p-322]-smooth_prefix[p-323];
  }
 }
 bool near(const Rect2i&r)const{
  int x0=r.position.x,z0=r.position.y,x1=x0+r.size.x+1,z1=z0+r.size.y+1;
  return smooth_prefix[z1*322+x1]-smooth_prefix[z0*322+x1]-smooth_prefix[z1*322+x0]+smooth_prefix[z0*322+x0]>0;
 }
 bool leaf(const Rect2i&r){
  bool sm=near(r);if(sm&&std::max(r.size.x,r.size.y)>20)return false;
  int x0=r.position.x,z0=r.position.y,x1=x0+r.size.x,z1=z0+r.size.y;
  const float* heights=samples.ptr();const float* ds=distances.ptr();
  int indices[4]={z0*STRIDE+x0,z0*STRIDE+x1,z1*STRIDE+x0,z1*STRIDE+x1};
  Vector4 h;
  for(int i=0;i<4;i++){double y=heights[indices[i]];if(ds[indices[i]]>smooth)y=std::floor(y/SPACING+0.5)*SPACING;h[i]=y;}
  double error=0,bound=sm?SMOOTH_ERROR:SURFACE_ERROR;
  for(int z=z0;z<=z1;z++)for(int x=x0;x<=x1;x++){
   Vector2 t=Vector2(x-x0,z-z0)/Vector2(r.size.x,r.size.y);
   double value;
   if(double(t.x)+double(t.y)<=1)value=double(h.x)+double(t.x)*(double(h.y)-h.x)+double(t.y)*(double(h.z)-h.x);
   else value=double(h.w)+(1-double(t.x))*(double(h.z)-h.w)+(1-double(t.y))*(double(h.y)-h.w);
   error=std::max(error,std::abs(value-heights[z*STRIDE+x]));
   if(error>bound && r.size!=Vector2i(1,1))return false;
  }
  Dictionary l;l["rect"]=Rect2i(coord*N+r.position,r.size);l["heights"]=h;
  int index=leaves.size();leaves.append(l);
  int32_t* o=owners.ptrw();
  for(int z=z0;z<z1;z++)for(int x=x0;x<x1;x++)o[z*N+x]=index;
  maximum_error=std::max(maximum_error,error);return true;
 }
 void partition(Rect2i r){
  if(leaf(r))return;
  int w0=std::max(1,r.size.x/2),h0=std::max(1,r.size.y/2);
  int widths[2]={w0,r.size.x-w0},heights[2]={h0,r.size.y-h0};
  int z=r.position.y;
  for(int j=0;j<(r.size.y>1?2:1);j++){
   int x=r.position.x;
   for(int i=0;i<(r.size.x>1?2:1);i++){partition(Rect2i(x,z,widths[i],heights[j]));x+=widths[i];}
   z+=heights[j];
  }
 }
 Dictionary result(bool source=true){
  Dictionary d;d["leaves"]=leaves;d["owners"]=owners;d["max_error"]=maximum_error;
  if(source){d["samples"]=samples;d["distances"]=distances;d["source_max_gradient"]=maximum_gradient;}
  return d;
 }
};
}
class TerrainBakeAccelerator:public RefCounted {
 GDCLASS(TerrainBakeAccelerator,RefCounted)
 std::vector<Segment> segments;
 std::unordered_map<uint64_t,std::vector<int>> grid;
 double amplitude=0,wavelength=220,half_width=3,blend=16;
 int seed=0;
 Ref<FastNoiseLite> point_noise;
 Vector2 sample(Vector2 point,const std::vector<int>&ids,const Ref<FastNoiseLite>&noise)const{
  double base=double(noise->get_noise_2d(point.x,point.y))*amplitude,nearest=INFINITY;
  for(int i:ids){const auto&s=segments[i];Vector2 p=point.clamp(s.a.min(s.b),s.a.max(s.b));if((point-p).length_squared()>nearest)continue;
   double t=std::clamp(double((point-s.a).dot(s.edge))/s.squared,0.0,1.0);
   nearest=std::min(nearest,double(point.distance_squared_to(s.a+s.edge*float(t))));
  }
  double total=0,weighted=0,reach=nearest+64;
  for(int i:ids){const auto&s=segments[i];Vector2 p=point.clamp(s.a.min(s.b),s.a.max(s.b));if((point-p).length_squared()>reach)continue;
   double t=std::clamp(double((point-s.a).dot(s.edge))/s.squared,0.0,1.0);
   double squared=point.distance_squared_to(s.a+s.edge*float(t));
   double compact=std::max(0.0,1.0-(squared-nearest)/64.0),weight=compact*compact/(squared+4);
   total+=weight;weighted+=weight*(double(s.height.x)+double(s.height.y)*t);
  }
  double target=total>0?weighted/total:base,distance=std::sqrt(nearest);
  double shoulder=std::max(0.0,distance-half_width-2),t=std::clamp(shoulder/blend,0.0,1.0);
  double weight=t*t*t*(t*(6*t-15)+10),allowance=shoulder*0.1;
  base=std::clamp(base,target-allowance,target+allowance);
  return Vector2(target+(base-target)*weight,distance);
 }
protected:
 static void _bind_methods(){
  ClassDB::bind_method(D_METHOD("configure","centers","grid","amplitude","wavelength","seed","half_width","blend"),&TerrainBakeAccelerator::configure);
  ClassDB::bind_method(D_METHOD("sample_point","point"),&TerrainBakeAccelerator::sample_point);
  ClassDB::bind_method(D_METHOD("sample_chunk","coord"),&TerrainBakeAccelerator::sample_chunk);
  ClassDB::bind_method(D_METHOD("refine_chunk","coord","samples","distances","leaves","requested"),&TerrainBakeAccelerator::refine_chunk);
  ClassDB::bind_method(D_METHOD("validate_top","rect","vertices","start","samples","distances","coord","smooth_distance"),&TerrainBakeAccelerator::validate_top);
 }
public:
 void configure(PackedVector3Array centers,Dictionary indices,double a,double wave,int s,double width,double b){
  amplitude=a;wavelength=wave;seed=s;half_width=width;blend=b;segments.clear();grid.clear();
  point_noise.instantiate();point_noise->set_seed(seed);point_noise->set_noise_type(FastNoiseLite::TYPE_SIMPLEX_SMOOTH);
  point_noise->set_fractal_type(FastNoiseLite::FRACTAL_FBM);point_noise->set_fractal_octaves(3);point_noise->set_fractal_gain(0.35);
  point_noise->set_frequency(1.0/wavelength);point_noise->set_offset(Vector3(331,0,719));
  const auto*p=centers.ptr();
  for(int i=0;i<centers.size()-1;i++){
   Segment v;v.a=Vector2(p[i].x,p[i].z);v.b=Vector2(p[i+1].x,p[i+1].z);v.edge=v.b-v.a;
   v.squared=v.edge.length_squared();v.height=Vector2(p[i].y,double(p[i+1].y)-p[i].y);segments.push_back(v);
  }
  Array keys=indices.keys();
  for(int i=0;i<keys.size();i++){Vector2i cell=keys[i];PackedInt32Array value=indices[keys[i]];grid[key(cell.x,cell.y)]=std::vector<int>(value.ptr(),value.ptr()+value.size());}
 }
 Vector2 sample_point(Vector2 point)const{
  auto found=grid.find(key(int(std::floor(point.x/32.0)),int(std::floor(point.y/32.0))));
  static const std::vector<int> empty;
  return sample(point,found==grid.end()?empty:found->second,point_noise);
 }
 Dictionary sample_chunk(Vector2i coord)const{
  Chunk c;c.coord=coord;c.smooth=half_width+2+blend;c.samples.resize(STRIDE*STRIDE);c.distances.resize(STRIDE*STRIDE);c.owners.resize(N*N);
  Ref<FastNoiseLite>noise;noise.instantiate();noise->set_seed(seed);noise->set_noise_type(FastNoiseLite::TYPE_SIMPLEX_SMOOTH);
  noise->set_fractal_type(FastNoiseLite::FRACTAL_FBM);noise->set_fractal_octaves(3);noise->set_fractal_gain(0.35);
  noise->set_frequency(1.0/wavelength);noise->set_offset(Vector3(331,0,719));
  std::vector<int>empty;auto found=grid.find(key(coord.x,coord.y));const auto&ids=found==grid.end()?empty:found->second;
  std::vector<std::vector<int>>queries(1024);
  for(int z=0;z<32;z++)for(int x=0;x<32;x++){
   Vector2 center=Vector2(coord)*32+Vector2(x+0.5,z+0.5);double nearest=INFINITY;std::vector<double>dist;
   for(int i:ids){double d=center.distance_to(closest(center,segments[i]));dist.push_back(d);nearest=std::min(nearest,d);}
   double radius=std::sqrt(0.5),reach=std::sqrt((nearest+radius)*(nearest+radius)+64)+radius;
   for(int i=0;i<int(ids.size());i++)if(dist[i]<=reach)queries[z*32+x].push_back(ids[i]);
  }
  float*h=c.samples.ptrw(),*d=c.distances.ptrw();
  for(int z=0;z<=N;z++)for(int x=0;x<=N;x++){
   Vector2 point=Vector2(coord*N+Vector2i(x,z))*float(SPACING);
   Vector2 v=sample(point,queries[std::min(z/10,31)*32+std::min(x/10,31)],noise);int i=z*STRIDE+x;h[i]=v.x;d[i]=v.y;
   if(!std::isfinite(v.x)){Dictionary error;error["error"]="Non-finite native terrain height.";return error;}
   if(x>0&&z>0)c.maximum_gradient=std::max(c.maximum_gradient,double(Vector2(double(v.x)-h[i-1],double(v.x)-h[i-STRIDE]).length())/SPACING);
  }
  c.prefix();c.partition(Rect2i(0,0,N,N));return c.result();
 }
 Dictionary refine_chunk(Vector2i coord,PackedFloat32Array samples,PackedFloat32Array distances,Array leaves,PackedInt32Array requested)const{
  Chunk c;c.coord=coord;c.smooth=half_width+2+blend;c.samples=samples;c.distances=distances;c.owners.resize(N*N);c.prefix();
  std::vector<bool> refine(leaves.size(),false);for(int i=0;i<requested.size();i++)refine[requested[i]]=true;
  for(int i=0;i<leaves.size();i++){
   Dictionary l=leaves[i];Rect2i global=l["rect"];Rect2i local(global.position-coord*N,global.size);
   if(refine[i]&&local.size!=Vector2i(1,1)){
    int w0=std::max(1,local.size.x/2),h0=std::max(1,local.size.y/2),z=local.position.y;
    int ws[2]={w0,local.size.x-w0},hs[2]={h0,local.size.y-h0};
    for(int j=0;j<(local.size.y>1?2:1);j++){int x=local.position.x;for(int k=0;k<(local.size.x>1?2:1);k++){c.partition(Rect2i(x,z,ws[k],hs[j]));x+=ws[k];}z+=hs[j];}
   }else if(!c.leaf(local))c.partition(local);
  }
  return c.result(false);
 }
 Dictionary validate_top(Rect2i rect,PackedVector3Array vertices,int start,PackedFloat32Array samples,PackedFloat32Array distances,Vector2i coord,double smooth)const{
  const auto*vtx=vertices.ptr();const auto*h=samples.ptr();const auto*d=distances.ptr();Vector2i origin=coord*N;bool good=true;double maximum=0;
  for(int i=start;i<vertices.size();i+=3){Vector3 a=vtx[i],b=vtx[i+1],c=vtx[i+2];Vector2 ab=Vector2(b.x-a.x,b.z-a.z),ac=Vector2(c.x-a.x,c.z-a.z);double inverse=1.0/ab.cross(ac);
   int x0=std::max(rect.position.x,int(std::ceil(std::min({a.x,b.x,c.x})/SPACING-0.001)));
   int z0=std::max(rect.position.y,int(std::ceil(std::min({a.z,b.z,c.z})/SPACING-0.001)));
   int x1=std::min(rect.position.x+rect.size.x,int(std::floor(std::max({a.x,b.x,c.x})/SPACING+0.001)));
   int z1=std::min(rect.position.y+rect.size.y,int(std::floor(std::max({a.z,b.z,c.z})/SPACING+0.001)));
   for(int z=z0;z<=z1;z++)for(int x=x0;x<=x1;x++){
    Vector2 ap=Vector2(x*SPACING-a.x,z*SPACING-a.z);double u=ap.cross(ac)*inverse,w=ab.cross(ap)*inverse;
    if(u<-0.0001||w<-0.0001||u+w>1.0001)continue;
    double y=double(a.y)+u*(double(b.y)-a.y)+w*(double(c.y)-a.y);int index=(z-origin.y)*STRIDE+x-origin.x;
    double error=std::abs(y-h[index]);maximum=std::max(maximum,error);double bound=d[index]<=smooth?SMOOTH_ERROR:SURFACE_ERROR;
    if(error>bound+0.0001)good=false;
   }
  }
  Dictionary result;result["good"]=good;result["max_error"]=maximum;return result;
 }
};
void initialize_native_bake(ModuleInitializationLevel level){if(level==MODULE_INITIALIZATION_LEVEL_SCENE)ClassDB::register_class<TerrainBakeAccelerator>();}
void uninitialize_native_bake(ModuleInitializationLevel){}
extern "C" GDExtensionBool GDE_EXPORT native_bake_init(GDExtensionInterfaceGetProcAddress address,GDExtensionClassLibraryPtr library,GDExtensionInitialization*initialization){
 GDExtensionBinding::InitObject init(address,library,initialization);init.register_initializer(initialize_native_bake);init.register_terminator(uninitialize_native_bake);init.set_minimum_library_initialization_level(MODULE_INITIALIZATION_LEVEL_SCENE);return init.init();
}
